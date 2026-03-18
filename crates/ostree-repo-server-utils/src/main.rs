//! `ostree-repo-server` — server-side OSTree repository utilities.
//!
//! # Subcommands
//!
//! | Subcommand   | Description |
//! |--------------|-------------|
//! | `sign`       | Sign an OSTree commit via an ED25519 key in AWS KMS and store the signature in the commit's detached metadata (`ostree.sign.ed25519`). |
//! | `export-key` | Export the ED25519 public key from KMS to a file in an ostree `--keys-dir` directory, enabling offline verification on client systems. |
//!
//! # Design notes
//!
//! * All ostree I/O uses `tokio::task::spawn_blocking` because the `ostree`
//!   Rust crate wraps synchronous GLib calls.
//! * KMS calls are async, executed directly on the Tokio runtime.
//! * The `sign` command opens the repository **twice** — once to read the
//!   commit and existing signatures, and once to write the updated list —
//!   because `ostree::Repo` cannot be transferred across the async/blocking
//!   boundary between the two `spawn_blocking` calls.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use ostree_repo_utils_common::{
    keys::{export_kms_public_key, write_pubkey_file},
    metadata::{read_ed25519_signatures, write_ed25519_signatures},
    repo::{load_commit_bytes, open_repo, resolve_ref},
    sign::{kms::KmsSigner, Signer as _},
};

#[derive(Parser)]
#[command(
    name = "ostree-repo-server",
    about = "OSTree repository server utilities"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sign an OSTree commit via a KMS-backed ED25519 key.
    Sign(SignArgs),
    /// Export a KMS public key to a keys-dir for offline verification.
    #[command(name = "export-key")]
    ExportKey(ExportKeyArgs),
}

#[derive(clap::Args)]
struct SignArgs {
    /// Signing backend provider (currently only `kms` is supported).
    #[arg(long, required = true)]
    provider: Provider,
    /// KMS key ID or ARN of the ED25519 signing key.
    #[arg(long, required = true)]
    key_id: String,
    /// Path to the OSTree repository.
    #[arg(long, required = true)]
    repo: PathBuf,
    /// Symbolic ref or commit checksum to sign.
    #[arg(long = "ref", required = true)]
    refspec: String,
    /// Replace all existing signatures instead of appending.
    ///
    /// By default the new signature is appended to any existing signatures
    /// for the commit, preserving multi-key and key-rotation scenarios.
    /// Pass `--replace` to discard all prior signatures and store only the
    /// new one (e.g. after a key has been revoked).
    #[arg(long)]
    replace: bool,
}

#[derive(clap::Args)]
struct ExportKeyArgs {
    /// Signing backend provider (currently only `kms` is supported).
    #[arg(long, required = true)]
    provider: Provider,
    /// KMS key ID or ARN of the ED25519 key to export.
    #[arg(long, required = true)]
    key_id: String,
    /// Version label for the key file name (e.g. `prod-v1` → `prod-v1.pub`).
    #[arg(long, required = true)]
    key_version: String,
    /// Directory to write the `.pub` file into (ostree `--keys-dir` path).
    #[arg(long, required = true)]
    keys_dir: PathBuf,
}

/// Signing backend selector. Extensible to `gpg`, `vault`, etc. in the future.
#[derive(Clone, ValueEnum)]
enum Provider {
    Kms,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Sign(args) => cmd_sign(args).await,
        Commands::ExportKey(args) => cmd_export_key(args).await,
    }
}

/// Handler for the `sign` subcommand.
///
/// Execution order:
/// 1. `spawn_blocking`: open repo, resolve ref, load commit bytes, read existing signatures.
/// 2. Async: call KMS `Sign` to produce the new signature.
/// 3. Build updated signature list (append or replace depending on `--replace`).
/// 4. `spawn_blocking`: open repo again, write updated signatures to detached metadata.
async fn cmd_sign(args: SignArgs) -> anyhow::Result<()> {
    let repo_path = args.repo.clone();
    let refspec = args.refspec.clone();

    // All ostree I/O is synchronous (GLib); run in a blocking thread.
    let (rev, commit_bytes, existing_sigs) =
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let repo = open_repo(&repo_path)?;
            let rev = resolve_ref(&repo, &refspec)?;
            let bytes = load_commit_bytes(&repo, &rev)?;
            let sigs = read_ed25519_signatures(&repo, &rev)?;
            Ok((rev, bytes, sigs))
        })
        .await??;

    // Sign via KMS (async).
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_kms::Client::new(&config);
    let signer = KmsSigner::new(args.key_id, client);
    let signature = signer.sign(&commit_bytes).await?;

    // Build the updated signature list.
    let new_sigs: Vec<[u8; 64]> = if args.replace {
        vec![signature]
    } else {
        let mut s = existing_sigs;
        s.push(signature);
        s
    };

    // Write back in a blocking thread.
    let repo_path = args.repo.clone();
    let rev_clone = rev.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let repo = open_repo(&repo_path)?;
        write_ed25519_signatures(&repo, &rev_clone, &new_sigs)?;
        Ok(())
    })
    .await??;

    println!("signed commit {rev}");
    Ok(())
}

/// Handler for the `export-key` subcommand.
///
/// Calls KMS `GetPublicKey`, strips the DER SPKI header to obtain the raw
/// 32-byte ED25519 key, and writes it base64-encoded to
/// `<keys-dir>/<key-version>.pub`.
async fn cmd_export_key(args: ExportKeyArgs) -> anyhow::Result<()> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_kms::Client::new(&config);

    let raw_key = export_kms_public_key(&args.key_id, &client).await?;
    write_pubkey_file(&raw_key, &args.keys_dir, &args.key_version)?;

    println!(
        "exported public key to {}/{}.pub",
        args.keys_dir.display(),
        args.key_version
    );
    Ok(())
}
