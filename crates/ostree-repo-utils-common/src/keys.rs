//! KMS public-key export for ostree offline verification.
//!
//! # Flow
//!
//! 1. Call AWS KMS `GetPublicKey` → DER-encoded SubjectPublicKeyInfo (SPKI).
//! 2. Validate and strip the fixed 12-byte ASN.1 header to extract the raw
//!    32-byte ED25519 public key.
//! 3. Base64-encode and write a single-line `.pub` file to a directory
//!    compatible with `ostree sign --verify --keys-dir=<dir>`.
//!
//! # SPKI structure
//!
//! RFC 8410 defines the ED25519 SPKI as a 44-byte DER blob:
//! ```text
//! SEQUENCE (42 bytes) {
//!   SEQUENCE (5 bytes) { OID 1.3.101.112 }   -- "id-Ed25519"
//!   BIT STRING (33 bytes) { 0x00 <32-byte key> }
//! }
//! ```
//! The 12-byte header is constant for all ED25519 keys, so we validate it
//! byte-for-byte rather than pulling in a full ASN.1 parser.

use std::path::Path;

use base64::Engine as _;

/// Fixed 12-byte DER header for an ED25519 SubjectPublicKeyInfo.
///
/// Hex: `30 2a 30 05 06 03 2b 65 70 03 21 00`
///
/// Decodes as:
/// - `30 2a` — SEQUENCE, 42 bytes
/// - `30 05` — SEQUENCE, 5 bytes
/// - `06 03 2b 65 70` — OID 1.3.101.112 (id-Ed25519)
/// - `03 21 00` — BIT STRING, 33 bytes, 0 unused bits (prefix for the 32-byte key)
const ED25519_SPKI_HEADER: &[u8] = &[
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// Fetch the ED25519 public key from KMS and return the raw 32-byte key material.
///
/// Calls `GetPublicKey` on `key_id`, validates the fixed 12-byte ASN.1 header
/// of the DER SPKI response, and extracts the trailing 32 bytes.
///
/// Returns an error if the KMS call fails, the response is missing the key
/// blob, or the blob does not conform to the expected ED25519 SPKI structure.
pub async fn export_kms_public_key(
    key_id: &str,
    client: &aws_sdk_kms::Client,
) -> anyhow::Result<[u8; 32]> {
    let response = client
        .get_public_key()
        .key_id(key_id)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("KMS GetPublicKey request failed: {e}"))?;

    let der_blob = response
        .public_key()
        .ok_or_else(|| anyhow::anyhow!("KMS response is missing the public_key field"))?;
    let der_bytes = der_blob.as_ref();

    parse_ed25519_spki(der_bytes)
}

/// Extract the raw 32-byte ED25519 public key from a DER SPKI blob.
///
/// Validates that the blob is exactly 44 bytes and begins with
/// [`ED25519_SPKI_HEADER`], then returns the trailing 32 bytes.
fn parse_ed25519_spki(der_bytes: &[u8]) -> anyhow::Result<[u8; 32]> {
    const TOTAL_LEN: usize = ED25519_SPKI_HEADER.len() + 32;

    if der_bytes.len() != TOTAL_LEN {
        return Err(anyhow::anyhow!(
            "unexpected SPKI length: {} bytes (expected {} for ED25519)",
            der_bytes.len(),
            TOTAL_LEN,
        ));
    }

    if &der_bytes[..ED25519_SPKI_HEADER.len()] != ED25519_SPKI_HEADER {
        return Err(anyhow::anyhow!(
            "unexpected SPKI header: {:02x?}\nexpected: {:02x?}\n\
             The KMS key may not be an ED25519 key.",
            &der_bytes[..ED25519_SPKI_HEADER.len()],
            ED25519_SPKI_HEADER,
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&der_bytes[ED25519_SPKI_HEADER.len()..]);
    Ok(key)
}

/// Base64-encode `raw_key` and write it as a single-line `.pub` file.
///
/// Writes to `<keys_dir>/<key_version>.pub`. The file format — a single
/// base64-encoded line with no trailing newline — is what `ostree sign
/// --verify --sign-type=ed25519 --keys-dir=<dir>` expects for each key.
///
/// An existing file at the target path is silently overwritten, supporting
/// key-rotation workflows.
pub fn write_pubkey_file(
    raw_key: &[u8; 32],
    keys_dir: &Path,
    key_version: &str,
) -> anyhow::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw_key);
    let path = keys_dir.join(format!("{key_version}.pub"));
    std::fs::write(&path, encoded)
        .map_err(|e| anyhow::anyhow!("writing public key to {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_spki_header_parses() {
        let mut der = Vec::new();
        der.extend_from_slice(ED25519_SPKI_HEADER);
        der.extend_from_slice(&[0xabu8; 32]);

        let key = parse_ed25519_spki(&der).expect("valid SPKI should parse");
        assert_eq!(key, [0xabu8; 32]);
    }

    #[test]
    fn wrong_header_returns_error() {
        let mut der = Vec::new();
        der.extend_from_slice(&[0x00u8; 12]); // wrong header
        der.extend_from_slice(&[0xabu8; 32]);

        let err = parse_ed25519_spki(&der).expect_err("wrong header should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("unexpected SPKI header"),
            "error message should mention SPKI header, got: {msg}"
        );
    }

    #[test]
    fn wrong_length_returns_error() {
        let err = parse_ed25519_spki(&[0u8; 10]).expect_err("short blob should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("unexpected SPKI length"),
            "error message should mention SPKI length, got: {msg}"
        );
    }
}
