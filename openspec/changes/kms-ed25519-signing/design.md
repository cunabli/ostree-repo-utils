## Context

The project is a greenfield Rust CLI toolset for OSTree repository management. The current codebase is a skeleton single-crate with no meaningful code. This change establishes the foundational workspace structure and implements the first real capability: signing OSTree commits using ED25519 keys backed by AWS KMS, producing signatures that are fully verifiable offline by the native `ostree` CLI.

Key constraints:
- Verification must work without KMS access (client systems are often air-gapped or offline)
- Signatures must be stored in ostree's native `ostree.sign.ed25519` detached metadata format for compatibility with `ostree sign --verify`
- Client systems must not carry any AWS SDK dependency
- Rust MSRV: 1.92 (do not bump)
- Target libostree: 2024.05 (Yocto build) and 2024.10 (current dev environment)

## Goals / Non-Goals

**Goals:**
- Establish a Cargo workspace with clean server/client/common separation
- Implement `ostree-repo-server sign --provider=kms` that produces ostree-compatible ED25519 signatures via KMS
- Implement `ostree-repo-server export-key --provider=kms` that writes a KMS public key to the ostree `--keys-dir` format for offline verification
- Async runtime (Tokio) in server binary, ready for future S3 operations
- Feature-gated common lib so `ostree-repo-client` has zero AWS transitive dependencies

**Non-Goals:**
- Custom verification subcommand (clients use native `ostree sign --verify`)
- S3 integration (workspace is structured for it, implementation deferred)
- TOML config for key aliases (deferred — CLI flags sufficient for now)
- Porting of releng scripts (workspace aligned for future iterations)

## Decisions

### D1: Cargo workspace with feature-gated common lib

**Decision:** Three crates — `ostree-repo-utils-common` (lib), `ostree-repo-server-utils` (bin → `ostree-repo-server`), `ostree-repo-client-utils` (bin → `ostree-repo-client`). Server enables `[features] default = ["kms"]` in common; client depends on common with `default-features = false`.

**Alternative considered:** Single crate with feature flags. Rejected because it puts the AWS SDK in the dependency graph for client builds even if feature-gated at the code level, and it complicates packaging (shipping a single binary without re-compiling).

**Binary naming:** Configured via `[[bin]] name = "ostree-repo-server"` in the server crate's `Cargo.toml`, keeping crate naming consistent (`ostree-repo-server-utils`) while producing clean installed binary names.

---

### D2: Provider as flag, not subcommand group

**Decision:** CLI shape is `sign --provider=kms` and `export-key --provider=kms`, not `kms sign` / `kms export-key`.

**Why:** Verbs are the primary dimension of the CLI; the signing backend is a detail. This allows adding `--provider=gpg` or `--provider=vault` later without restructuring the command tree. It also mirrors how `ostree sign --sign-type=ed25519` works.

---

### D3: Store signatures in `ostree.sign.ed25519` with GVariant type `(aay)`

**Decision:** Use ostree's native ed25519 detached metadata key and exact GVariant type.

**Why:** Clients verify with `ostree sign --verify --sign-type=ed25519 --keys-dir=...` without needing our tool. This is the only path to offline-verification-without-our-binary.

**Critical implementation detail:** The GVariant type is `(aay)` — a tuple wrapping an array of byte arrays. Not `aay`. Getting this wrong causes silent verification failure. Each inner `ay` is the raw 64-byte ED25519 signature.

**Append vs replace:** Default behavior appends to existing signatures (preserving multi-key and key-rotation scenarios). `--replace` flag replaces all existing signatures on the commit.

---

### D4: KMS signing algorithm `ED25519_SHA_512` with `MessageType::Raw`

**Decision:** Use KMS signing algorithm `ED25519_SHA_512` with `MessageType::Raw`, passing the raw serialized commit GVariant bytes.

**Rationale from KMS docs:** The KMS API supports two ED25519 algorithms:
- `ED25519_SHA_512`: standard ED25519 — **requires `MessageType::Raw`** (KMS performs the internal two-pass SHA-512 hashing per RFC 8032)
- `ED25519_PH_SHA_512`: ED25519ph prehash variant — accepts `MessageType::Digest`

We must use `ED25519_SHA_512` / `MessageType::Raw` because:
1. KMS explicitly mandates it for this algorithm — `MessageType::Digest` is not accepted
2. More importantly: `ED25519_PH_SHA_512` produces an ED25519ph signature, which is a **different algorithm** from standard ED25519. ostree's verification (via libsodium `crypto_sign_verify_detached`) implements standard ED25519, not ED25519ph — using the prehash variant would produce signatures that fail offline verification

**Size constraint:** KMS Raw messages are limited to 4096 bytes. OSTree commit GVariants are typically 200–600 bytes. Validate at runtime and surface a clear error if exceeded.

---

### D5: Public key export via DER SPKI stripping

**Decision:** `GetPublicKey` returns DER-encoded SubjectPublicKeyInfo. The raw 32-byte public key is the last 32 bytes of the response (after the fixed 12-byte ASN.1 header: `30 2a 30 05 06 03 2b 65 70 03 21 00`). Base64-encode and write as a single line to `<keys-dir>/<key-version>.pub`.

**Alternative considered:** Use the `der` crate for full ASN.1 parsing. Acceptable but adds a dependency for a fixed-structure operation. The ED25519 SPKI header is always exactly 12 bytes (`SubjectPublicKeyInfo` with OID `1.3.101.112`). Prefer manual extraction with a validation assertion on the header bytes, with a descriptive error if the structure does not match expectations.

**Key version naming:** Explicit `--key-version` flag (e.g., `prod-v1`). Output file: `<keys-dir>/prod-v1.pub`. No magic derivation from ARN or alias — operator decides the naming scheme.

---

### D6: Synchronous ostree calls wrapped in `spawn_blocking`

**Decision:** The `ostree` Rust crate is GObject-based and synchronous. All ostree I/O runs inside `tokio::task::spawn_blocking` to avoid blocking the async executor.

**Why:** The server binary is async-first for KMS and future S3 calls. Blocking the Tokio executor on GLib I/O would stall the runtime. `spawn_blocking` is the standard Tokio pattern for sync I/O.

---

### D7: Dependency versions pinned to Rust 1.92 MSRV

Key crate MSRV notes:
- `ostree = "0.20"`: part of gtk-rs ecosystem, MSRV ~1.70. Compatible.
- `aws-sdk-kms = "1"`: AWS SDK Rust MSRV ~1.82. Compatible.
- `tokio`: current stable MSRV is 1.70. Compatible.
- `clap = "4"` with derive: MSRV 1.74. Compatible.

No MSRV bumps required. Verify with `cargo msrv` or by checking each crate's `Cargo.toml` during implementation.

---

## Risks / Trade-offs

**[Risk] GVariant `(aay)` type mismatch** → `ostree sign --verify` silently finds no signatures if the type string is wrong. Mitigation: integration test that writes a signature and reads it back through the ostree CLI.

**[Risk] KMS `ED25519_SHA_512` message size limit (4096 bytes)** → Fails for pathologically large commit GVariants. Mitigation: check `commit_bytes.len()` before calling KMS, return a descriptive error referencing the constraint.

**[Risk] AWS credential availability in pipeline** → `sign` and `export-key` require valid AWS credentials. Mitigation: AWS SDK credential chain (env vars, instance role, `~/.aws`) handles this transparently. Document required IAM permissions: `kms:Sign`, `kms:GetPublicKey`.

**[Risk] ostree crate GLib runtime requirements** → `ostree = "0.20"` requires a GLib/GObject runtime and `libostree` shared library at link/runtime. Mitigation: document minimum libostree version (≥ 2024.05); the Yocto and dev environments both satisfy this.

**[Trade-off] Two separate binaries vs one** → Slightly more packaging surface, but necessary to avoid AWS SDK on client systems. The client binary remains lean and auditable.

## Migration Plan

1. Delete `src/main.rs` and convert root `Cargo.toml` from `[package]` to `[workspace]` with `members = ["crates/*"]`
2. Scaffold three crates under `crates/`
3. No data migration needed — new detached metadata keys are additive to any existing ostree repository

## Open Questions

- Should `export-key` also print the base64 public key to stdout (for piping into other tools) in addition to writing the file? Low cost, useful for automation.
- Do we need a `--dry-run` flag for `sign` to validate KMS connectivity and commit resolution without writing metadata?
