## 1. Workspace Restructure

- [ ] 1.1 Replace root `Cargo.toml` `[package]` section with `[workspace]` manifest pointing to `members = ["crates/*"]`
- [ ] 1.2 Create `crates/ostree-repo-utils-common/` lib crate with `Cargo.toml` declaring `[features] kms = ["dep:aws-sdk-kms", "dep:aws-config"]`
- [ ] 1.3 Create `crates/ostree-repo-server-utils/` binary crate with `Cargo.toml` declaring `[[bin]] name = "ostree-repo-server"` and dependency on common with `features = ["kms"]`
- [ ] 1.4 Create `crates/ostree-repo-client-utils/` binary crate with `Cargo.toml` declaring `[[bin]] name = "ostree-repo-client"` and dependency on common with `default-features = false`
- [ ] 1.5 Remove old `src/main.rs` and verify `cargo build --workspace` compiles all three crates
- [ ] 1.6 Verify client binary has no AWS transitive deps: `cargo tree -p ostree-repo-client-utils | grep aws` should produce no output

## 2. Common Library: Repo and Metadata Primitives

- [ ] 2.1 Add `ostree`, `glib` dependencies to common `Cargo.toml`
- [ ] 2.2 Implement `src/repo.rs`: `open_repo(path) -> Result<Repo>`, `resolve_ref(repo, refspec) -> Result<String>`, `load_commit_bytes(repo, rev) -> Result<Vec<u8>>`
- [ ] 2.3 Implement `src/metadata.rs`: `read_ed25519_signatures(repo, rev) -> Result<Vec<[u8; 64]>>` and `write_ed25519_signatures(repo, rev, sigs: &[[u8; 64]]) -> Result<()>` using GVariant type `(aay)`
- [ ] 2.4 Write unit test confirming the GVariant type string `(aay)` round-trips correctly through read/write

## 3. Common Library: KMS Signing (feature = "kms")

- [ ] 3.1 Add `aws-sdk-kms`, `aws-config`, `tokio` (as optional/feature-gated where applicable) to common `Cargo.toml` under the `kms` feature
- [ ] 3.2 Implement `src/sign/mod.rs`: define async `Signer` trait with `sign(commit_bytes: &[u8]) -> Result<[u8; 64]>`
- [ ] 3.3 Implement `src/sign/kms.rs` (`#[cfg(feature = "kms")]`): `KmsSigner { key_id: String, client: aws_sdk_kms::Client }` implementing `Signer` — call KMS `Sign` with `SigningAlgorithm::Ed25519Sha512` and `MessageType::Raw`
- [ ] 3.4 Add commit byte size guard: return `Err` with descriptive message if `commit_bytes.len() > 4096`
- [ ] 3.5 Validate KMS response is exactly 64 bytes; return `Err` if not

## 4. Common Library: Key Export (feature = "kms")

- [ ] 4.1 Implement `src/keys.rs` (`#[cfg(feature = "kms")]`): `export_kms_public_key(key_id, client) -> Result<[u8; 32]>` — call `GetPublicKey`, validate the 12-byte ED25519 SPKI header (`30 2a 30 05 06 03 2b 65 70 03 21 00`), extract trailing 32 bytes
- [ ] 4.2 Implement `write_pubkey_file(raw_key: &[u8; 32], keys_dir: &Path, key_version: &str) -> Result<()>` — base64-encode and write single-line `.pub` file
- [ ] 4.3 Write unit test for SPKI header validation: correct header parses, wrong header returns descriptive error

## 5. Server Binary: CLI and Command Wiring

- [ ] 5.1 Add `clap` with `derive` feature to server `Cargo.toml`; add `tokio` with `full` features
- [ ] 5.2 Define `main.rs` with `#[tokio::main]` and top-level clap `Cli` struct with `sign` and `export-key` subcommands
- [ ] 5.3 Define `SignArgs`: `--provider` (required, enum), `--key-id` (required), `--repo` (required), `--ref` (required), `--replace` (flag)
- [ ] 5.4 Implement `sign` command handler: open repo via `spawn_blocking`, resolve ref, load commit bytes, call `KmsSigner`, append/replace signature in detached metadata
- [ ] 5.5 Define `ExportKeyArgs`: `--provider` (required, enum), `--key-id` (required), `--key-version` (required), `--keys-dir` (required)
- [ ] 5.6 Implement `export-key` command handler: call `export_kms_public_key`, call `write_pubkey_file`
- [ ] 5.7 Return exit code 1 with stderr message for all error paths; exit code 0 with confirmation message on success

## 6. Integration Verification

- [ ] 6.1 Manual smoke test: sign a local ostree commit with a real or mocked KMS response; verify signature appears in detached metadata with correct GVariant type
- [ ] 6.2 End-to-end test: export public key → sign commit → run `ostree sign --verify --sign-type=ed25519 --keys-dir=...` and confirm exit 0
- [ ] 6.3 Verify `--replace` flag: sign twice with different (mocked) keys, confirm only the second signature remains
- [ ] 6.4 Verify append default: sign twice, confirm both signatures present
- [ ] 6.5 Confirm `cargo +1.92 build --workspace` passes (MSRV check)
