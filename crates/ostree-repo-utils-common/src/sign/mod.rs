//! Signing abstraction and backend implementations.
//!
//! [`Signer`] is the core trait. The only current implementation is
//! [`kms::KmsSigner`], gated behind the `kms` feature.
//!
//! Adding a new backend (e.g. a local key file or HashiCorp Vault) means
//! implementing [`Signer`] in a new submodule and wiring it into the server
//! CLI's `--provider` enum.

#[cfg(feature = "kms")]
pub mod kms;

/// Signs raw commit bytes, producing a 64-byte ED25519 signature.
///
/// Implementations must accept the raw serialized GVariant bytes of an
/// OSTree commit object (as returned by [`crate::repo::load_commit_bytes`])
/// and produce a standard ED25519 signature over those bytes.
///
/// The returned signature is stored verbatim as one `ay` element inside the
/// `(aay)` GVariant written to `ostree.sign.ed25519` detached metadata.
///
/// # Note on `impl Future` vs `async fn`
///
/// The method deliberately returns `impl Future` rather than using `async fn`
/// in the trait. This keeps the trait compatible with bounds like `Send` on
/// the returned future, which would otherwise be impossible to express or
/// relax without a breaking change.
pub trait Signer {
    fn sign(
        &self,
        commit_bytes: &[u8],
    ) -> impl std::future::Future<Output = anyhow::Result<[u8; 64]>>;
}
