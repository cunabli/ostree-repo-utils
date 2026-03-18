## Why

The existing `ostree sign` CLI only supports local key material, making it unsuitable for production release pipelines where private keys must be protected in a hardware-backed KMS. This change introduces server-side signing via AWS KMS ED25519 keys while preserving fully offline verification on client systems — removing any KMS dependency from the client path.

## What Changes

- **BREAKING**: Converts the current single-crate layout into a Cargo workspace with three crates (`ostree-repo-utils-common`, `ostree-repo-server-utils`, `ostree-repo-client-utils`). The existing `src/main.rs` skeleton is replaced.
- Introduces `sign` subcommand in the server binary: signs an OSTree commit via AWS KMS ED25519, storing the signature in the commit's detached metadata under `ostree.sign.ed25519` — compatible with `ostree sign --verify`.
- Introduces `export-key` subcommand in the server binary: exports the ED25519 public key from KMS in a format compatible with ostree's `--keys-dir` for offline verification.
- Establishes the `ostree-repo-utils-common` lib with feature gates (`kms`, `s3`) so the client binary carries no AWS dependencies.

## Capabilities

### New Capabilities

- `workspace-structure`: Cargo workspace layout with three crates, feature-gated common lib, and async (Tokio) runtime in the server binary.
- `kms-commit-signing`: Sign an OSTree commit's GVariant bytes via AWS KMS ED25519 and write the signature into detached metadata (`ostree.sign.ed25519`, GVariant type `(aay)`).
- `kms-key-export`: Export an ED25519 public key from KMS (DER SPKI → raw 32 bytes → base64) to a file in a `--keys-dir` directory for ostree-compatible offline verification.

### Modified Capabilities

## Impact

- **Cargo workspace**: Root `Cargo.toml` becomes a workspace manifest; `src/` is replaced by `crates/` with three members.
- **New dependencies**: `ostree = "0.20"`, `clap` (derive), `aws-sdk-kms = "1"` (feature-gated), `tokio` (server only), `base64`, `der` or manual SPKI parsing.
- **Build targets**: Two installable binaries — `ostree-repo-server` (pipeline/build infra, from crate `ostree-repo-server-utils`) and `ostree-repo-client` (target/client systems, from crate `ostree-repo-client-utils`).
- **No runtime AWS dependency on clients**: The client binary compiles without any AWS crates.
