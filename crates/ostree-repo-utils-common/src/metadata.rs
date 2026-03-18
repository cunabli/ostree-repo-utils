//! Read and write ED25519 signatures in an OSTree commit's detached metadata.
//!
//! # Storage format
//!
//! OSTree stores per-commit metadata as an `a{sv}` GVariant (a dictionary
//! mapping strings to arbitrary variants). The ED25519 signature list lives
//! under the key `"ostree.sign.ed25519"` with GVariant type **`(aay)`** — a
//! 1-tuple wrapping an array of byte arrays, where each inner `ay` is a
//! raw 64-byte signature.
//!
//! Getting this type string exactly right matters: `ostree sign --verify`
//! looks for `(aay)`, not `aay`. A mismatch causes silent verification
//! failure.
//!
//! # Append-by-default behaviour
//!
//! [`write_ed25519_signatures`] accepts the *complete* intended signature list.
//! The `sign` command in the server binary is responsible for deciding whether
//! to append to or replace the existing list (controlled by the `--replace`
//! flag). This module only performs the raw read/write.

use anyhow::Context as _;
use glib::prelude::*;

/// GLib detached metadata key for ED25519 signatures, as used by `ostree sign`.
const SIGN_ED25519_KEY: &str = "ostree.sign.ed25519";

/// Read all ED25519 signatures stored in a commit's detached metadata.
///
/// Returns an empty vec if the commit has no detached metadata at all, or if
/// the `ostree.sign.ed25519` key is absent from the metadata dictionary.
///
/// Each returned element is a raw 64-byte ED25519 signature. Returns an error
/// if any entry has a length other than 64 bytes.
pub fn read_ed25519_signatures(repo: &ostree::Repo, rev: &str) -> anyhow::Result<Vec<[u8; 64]>> {
    let metadata = repo
        .read_commit_detached_metadata(rev, None::<&gio::Cancellable>)
        .with_context(|| format!("reading detached metadata for {rev}"))?;

    let metadata = match metadata {
        Some(m) => m,
        None => return Ok(vec![]),
    };

    // Use VariantDict to look up the (aay) value from the a{sv} dict.
    let dict = glib::VariantDict::new(Some(&metadata));
    let sig_variant: Option<glib::Variant> = dict.lookup_value(SIGN_ED25519_KEY, None);
    let sig_variant = match sig_variant {
        Some(v) => v,
        None => return Ok(vec![]),
    };

    // sig_variant is (aay): a tuple containing an array of byte arrays.
    // child_value(0) gives the aay (the inner array of signatures).
    let aay = sig_variant.child_value(0);
    let n = aay.n_children();
    let mut sigs = Vec::with_capacity(n);
    for i in 0..n {
        let ay = aay.child_value(i);
        let bytes: Vec<u8> = ay
            .get()
            .ok_or_else(|| anyhow::anyhow!("signature {i} has unexpected GVariant type"))?;
        let sig: [u8; 64] = bytes
            .try_into()
            .map_err(|v: Vec<u8>| anyhow::anyhow!("signature {i} has wrong length: {}", v.len()))?;
        sigs.push(sig);
    }
    Ok(sigs)
}

/// Write `sigs` into a commit's detached metadata under `"ostree.sign.ed25519"`.
///
/// All other metadata keys on the commit are preserved. Any pre-existing
/// `ostree.sign.ed25519` entry is replaced by the provided list; callers are
/// responsible for merging with existing signatures if append semantics are
/// desired (see the `sign` command handler in the server binary).
///
/// The signatures are stored as a GVariant of type `(aay)` — required by
/// `ostree sign --verify` for ED25519.
pub fn write_ed25519_signatures(
    repo: &ostree::Repo,
    rev: &str,
    sigs: &[[u8; 64]],
) -> anyhow::Result<()> {
    let existing = repo
        .read_commit_detached_metadata(rev, None::<&gio::Cancellable>)
        .with_context(|| format!("reading detached metadata for {rev}"))?;

    // Vec<Vec<u8>>.to_variant() produces aay; the 1-tuple wraps it as (aay).
    let sig_vecs: Vec<Vec<u8>> = sigs.iter().map(|s| s.to_vec()).collect();
    let tuple_variant: glib::Variant = (sig_vecs,).to_variant();

    // Merge into the existing a{sv} dict (or create a fresh one).
    let dict = glib::VariantDict::new(existing.as_ref());
    dict.insert_value(SIGN_ED25519_KEY, &tuple_variant);
    let new_metadata = dict.end();

    repo.write_commit_detached_metadata(rev, Some(&new_metadata), None::<&gio::Cancellable>)
        .with_context(|| format!("writing detached metadata for {rev}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use glib::prelude::*;

    /// Verify that our (aay) encoding round-trips correctly through GVariant
    /// serialization and deserialization, including the type string check.
    /// This guards against the most dangerous failure mode: silently storing
    /// signatures under a type string that `ostree sign --verify` won't find.
    #[test]
    fn aay_gvariant_type_roundtrips() {
        let sig1 = [0xabu8; 64];
        let sig2 = [0xcdu8; 64];
        let sigs: &[[u8; 64]] = &[sig1, sig2];

        // Build (aay) the same way write_ed25519_signatures does.
        let sig_vecs: Vec<Vec<u8>> = sigs.iter().map(|s| s.to_vec()).collect();
        let tuple_variant: glib::Variant = (sig_vecs.clone(),).to_variant();

        assert_eq!(tuple_variant.type_().as_str(), "(aay)");
        assert_eq!(tuple_variant.n_children(), 1);

        let aay = tuple_variant.child_value(0);
        assert_eq!(aay.n_children(), 2);

        for (i, expected) in sigs.iter().enumerate() {
            let ay = aay.child_value(i);
            let bytes: Vec<u8> = ay.get().expect("should deserialize ay to Vec<u8>");
            assert_eq!(bytes.len(), 64);
            assert_eq!(bytes, expected.to_vec());
        }
    }
}
