//! AWS KMS backend for the [`Signer`] trait.
//!
//! # KMS algorithm choice
//!
//! Uses `ED25519_SHA_512` with `MessageType::Raw`. KMS mandates `Raw` for
//! this algorithm — `Digest` is rejected. More importantly, the alternative
//! `ED25519_PH_SHA_512` produces an ED25519**ph** (prehash) signature, which
//! is a *different* algorithm that libsodium's `crypto_sign_verify_detached`
//! (used by `ostree sign --verify`) does not accept.
//!
//! # Message size limit
//!
//! KMS Raw messages are capped at 4096 bytes. OSTree commit GVariants are
//! typically 200–600 bytes, so this limit is checked defensively before
//! making the API call to surface a clear error if ever exceeded.

use aws_sdk_kms::types::{MessageType, SigningAlgorithmSpec};

use super::Signer;

/// An ED25519 [`Signer`] backed by an AWS KMS key.
///
/// The `client` must already be configured with appropriate credentials and
/// region. The referenced KMS key must be an asymmetric ED25519 key with the
/// `Sign` permission granted to the caller.
pub struct KmsSigner {
    /// KMS key identifier — an alias (`alias/my-key`), key ID, or full ARN.
    key_id: String,
    client: aws_sdk_kms::Client,
}

impl KmsSigner {
    /// Create a new `KmsSigner` for the given KMS key.
    pub fn new(key_id: String, client: aws_sdk_kms::Client) -> Self {
        Self { key_id, client }
    }
}

impl Signer for KmsSigner {
    async fn sign(&self, commit_bytes: &[u8]) -> anyhow::Result<[u8; 64]> {
        // KMS Raw messages are limited to 4096 bytes. OSTree commits are
        // typically well under 1 KiB; this check guards against pathological
        // cases and surfaces a clear error before an opaque KMS rejection.
        if commit_bytes.len() > 4096 {
            return Err(anyhow::anyhow!(
                "commit GVariant is {} bytes, exceeding the KMS 4096-byte Raw message size limit",
                commit_bytes.len()
            ));
        }

        let response = self
            .client
            .sign()
            .key_id(&self.key_id)
            .message(aws_sdk_kms::primitives::Blob::new(commit_bytes.to_vec()))
            // Raw: KMS performs the internal two-pass SHA-512 per RFC 8032.
            .message_type(MessageType::Raw)
            // ED25519_SHA_512 = standard ED25519, compatible with libsodium verify.
            .signing_algorithm(SigningAlgorithmSpec::Ed25519Sha512)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("KMS Sign request failed: {e}"))?;

        let sig_blob = response
            .signature()
            .ok_or_else(|| anyhow::anyhow!("KMS response is missing the signature field"))?;
        let sig_bytes = sig_blob.as_ref();

        // A well-formed ED25519 signature is always exactly 64 bytes.
        if sig_bytes.len() != 64 {
            return Err(anyhow::anyhow!(
                "KMS returned a signature of {} bytes; expected exactly 64 bytes for ED25519",
                sig_bytes.len()
            ));
        }

        let mut sig = [0u8; 64];
        sig.copy_from_slice(sig_bytes);
        Ok(sig)
    }
}
