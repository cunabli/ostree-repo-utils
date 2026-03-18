//! Shared primitives for `ostree-repo-server` and `ostree-repo-client`.
//!
//! # Feature flags
//!
//! | Feature | What it enables |
//! |---------|-----------------|
//! | `kms`   | [`keys`] and [`sign::kms`] — AWS KMS signing and public-key export. Pulls in `aws-sdk-kms` and `aws-config`. Enabled by default in the server crate; **not** enabled in the client crate so the client carries zero AWS transitive dependencies. |
//!
//! # Module overview
//!
//! * [`repo`]     — Open an OSTree repository and load commit objects (thin wrappers around `ostree::Repo`).
//! * [`metadata`] — Read and write ED25519 signatures stored in a commit's detached metadata (`ostree.sign.ed25519`, GVariant type `(aay)`).
//! * [`sign`]     — [`sign::Signer`] trait and, when `kms` is enabled, [`sign::kms::KmsSigner`].
//! * [`keys`]     — *(kms feature)* Export a KMS public key and write it to an ostree `--keys-dir` directory.

pub mod metadata;
pub mod repo;
pub mod sign;

#[cfg(feature = "kms")]
pub mod keys;
