# Contribution guidelines

First off, thank you for considering contributing to ostree-repo-utils.

If your contribution is not straightforward, please first discuss the change you
wish to make by creating a new issue before making the change.

## Reporting issues

Before reporting an issue on the
[issue tracker](https://github.com/cunabli/ostree-repo-utils/issues),
please check that it has not already been reported by searching for some related
keywords.

## Pull requests

Try to do one pull request per change.

### Updating the changelog

Update the changes you have made in
[CHANGELOG](https://github.com/cunabli/ostree-repo-utils/blob/main/CHANGELOG.md)
file under the **Unreleased** section.

Add the changes of your pull request to one of the following subsections,
depending on the types of changes defined by
[Keep a changelog](https://keepachangelog.com/en/1.0.0/):

- `Added` for new features.
- `Changed` for changes in existing functionality.
- `Deprecated` for soon-to-be removed features.
- `Removed` for now removed features.
- `Fixed` for any bug fixes.
- `Security` in case of vulnerabilities.

If the required subsection does not exist yet under **Unreleased**, create it!

## Developing

### Workspace layout

```
Cargo.toml                          # workspace root — no [package], only [workspace]
crates/
  ostree-repo-utils-common/         # shared library (lib crate)
    src/
      lib.rs                        # module declarations + crate-level docs
      repo.rs                       # open_repo, resolve_ref, load_commit_bytes
      metadata.rs                   # read/write ostree.sign.ed25519 detached metadata
      sign/
        mod.rs                      # Signer trait
        kms.rs                      # KmsSigner (feature = "kms")
      keys.rs                       # export_kms_public_key, write_pubkey_file (feature = "kms")
  ostree-repo-server-utils/         # server binary crate → ostree-repo-server
    src/main.rs                     # CLI (clap), sign + export-key command handlers
  ostree-repo-client-utils/         # client binary crate → ostree-repo-client
    src/main.rs                     # placeholder (client subcommands TBD)
```

#### Feature flags

The `kms` feature on `ostree-repo-utils-common` gates all AWS SDK code:

| Crate | `kms` feature |
|-------|---------------|
| `ostree-repo-utils-common` | **optional** — disabled by default |
| `ostree-repo-server-utils` | always enabled (`features = ["kms"]` in its `Cargo.toml`) |
| `ostree-repo-client-utils` | never enabled (`default-features = false`) |

This keeps the client binary free of any AWS transitive dependency. When adding code that touches AWS, always place it behind `#[cfg(feature = "kms")]` or inside a module that is only declared under that flag.

### Prerequisites

- **Rust 1.92+** (MSRV — do not use language or library features requiring a newer compiler).
- **libostree** development headers — `ostree-1.pc` must be on `PKG_CONFIG_PATH`.
  - Fedora/RHEL: `dnf install ostree-devel`
  - Debian/Ubuntu: `apt install libostree-dev`
- **pkg-config**

### Set up

```shell
git clone https://github.com/cunabli/ostree-repo-utils
cd ostree-repo-utils
cargo build --workspace
```

### Useful commands

- Build all crates:

  ```shell
  cargo build --workspace
  ```

- Build release binaries:

  ```shell
  cargo build --workspace --release
  ```

- Run unit tests:

  ```shell
  # Without kms feature (metadata + GVariant tests):
  cargo test -p ostree-repo-utils-common

  # With kms feature (adds SPKI header validation tests):
  cargo test -p ostree-repo-utils-common --features kms

  # All crates, all features:
  cargo test --workspace --all-features
  ```

- Run Clippy:

  ```shell
  cargo clippy --all-targets --all-features --workspace
  ```

- Check formatting:

  ```shell
  cargo fmt --all -- --check
  ```

- Format the code:

  ```shell
  cargo fmt --all
  ```

- Check documentation (zero warnings expected):

  ```shell
  cargo doc --workspace --no-deps
  ```

- MSRV check:

  ```shell
  cargo +1.92 build --workspace
  # Install the toolchain if needed: rustup toolchain install 1.92
  ```

  Do not bump the MSRV without a deliberate decision — the target Yocto build environment constrains the available Rust version.

### Integration tests

Unit tests do not require real AWS credentials or a running OSTree repository. Integration verification (sign → export-key → `ostree sign --verify`) requires:

1. A local OSTree repository with at least one commit.
2. Real AWS KMS credentials or a local mock (e.g. LocalStack).
3. The `ostree` CLI installed on the test machine.

```shell
# Create a scratch repo and commit
mkdir /tmp/test-repo
ostree init --repo /tmp/test-repo
echo hello > /tmp/hello
ostree commit --repo /tmp/test-repo --branch main /tmp/hello

# Sign it
ostree-repo-server sign \
  --provider kms --key-id alias/test-key \
  --repo /tmp/test-repo --ref main

# Export the public key
mkdir /tmp/keys
ostree-repo-server export-key \
  --provider kms --key-id alias/test-key \
  --key-version test-v1 --keys-dir /tmp/keys

# Verify offline
ostree sign --verify --sign-type ed25519 \
  --keys-dir /tmp/keys --repo /tmp/test-repo \
  $(ostree rev-parse --repo /tmp/test-repo main)
```

### Adding a new signing backend

The `--provider` flag on both `sign` and `export-key` selects the signing backend. To add a new one (e.g. HashiCorp Vault):

1. **Add a feature flag** in `ostree-repo-utils-common/Cargo.toml`:
   ```toml
   vault = ["dep:some-vault-crate"]
   ```

2. **Implement `Signer`** in a new module `crates/ostree-repo-utils-common/src/sign/vault.rs` behind `#[cfg(feature = "vault")]`.

3. **Declare the module** in `sign/mod.rs`:
   ```rust
   #[cfg(feature = "vault")]
   pub mod vault;
   ```

4. **Add the variant** to the `Provider` enum in `ostree-repo-server-utils/src/main.rs`:
   ```rust
   #[derive(Clone, ValueEnum)]
   enum Provider {
       Kms,
       Vault,
   }
   ```

5. **Wire it into `cmd_sign`** (and `cmd_export_key` if the backend supports key export) by matching on the new variant.

6. **Enable the feature** in `ostree-repo-server-utils/Cargo.toml`.

#### `Signer` contract

- Accept the raw serialized GVariant bytes of an OSTree commit (≤ 4096 bytes typical).
- Return exactly 64 bytes — a standard ED25519 signature verifiable by libsodium's `crypto_sign_verify_detached`.
- All I/O should be async; the Tokio runtime is provided by the server binary.
- GLib/ostree calls must be wrapped in `tokio::task::spawn_blocking`.

### Adding a new subcommand

1. Add a new `Args` struct and a variant to `Commands` in `main.rs`.
2. Implement the async handler function (`cmd_<name>`).
3. Match the new variant in `run()`.
4. Add any new library functions to the appropriate module in `ostree-repo-utils-common`.
5. Update `README.md` with usage examples.
