//! Thin wrappers around `ostree::Repo` for opening repositories and loading commit objects.
//!
//! All functions are **synchronous** (the underlying `ostree` crate is GLib-based and blocking).
//! Call them from inside `tokio::task::spawn_blocking` when used from an async context.

use anyhow::Context as _;
use std::path::Path;

/// Open an OSTree repository at `path`.
///
/// Constructs an `ostree::Repo` backed by the directory at `path` and calls
/// `open()` to verify the repo is accessible and well-formed.
pub fn open_repo(path: &Path) -> anyhow::Result<ostree::Repo> {
    let file = gio::File::for_path(path);
    let repo = ostree::Repo::new(&file);
    repo.open(None::<&gio::Cancellable>)
        .with_context(|| format!("opening ostree repo at {}", path.display()))?;
    Ok(repo)
}

/// Resolve a symbolic ref or branch name to its full commit checksum.
///
/// `refspec` may be a branch name (e.g. `"main"`), a remote ref
/// (`"origin:main"`), or an already-resolved 64-hex checksum — ostree
/// handles all of these through `resolve_rev`. Returns an error if the ref
/// does not exist.
pub fn resolve_ref(repo: &ostree::Repo, refspec: &str) -> anyhow::Result<String> {
    let rev = repo
        .resolve_rev(refspec, false)
        .with_context(|| format!("resolving ref {refspec:?}"))?
        .ok_or_else(|| anyhow::anyhow!("ref {refspec:?} not found in repo"))?;
    Ok(rev.to_string())
}

/// Load the serialized GVariant bytes for the commit object identified by `rev`.
///
/// `rev` must be a fully-resolved commit checksum (use [`resolve_ref`] first
/// for symbolic refs). The returned bytes are the raw GVariant serialization
/// of the `OstreeCommit` tuple — these are the bytes passed to KMS for signing,
/// and the bytes that ostree itself hashes/verifies.
pub fn load_commit_bytes(repo: &ostree::Repo, rev: &str) -> anyhow::Result<Vec<u8>> {
    let variant = repo
        .load_variant(ostree::ObjectType::Commit, rev)
        .with_context(|| format!("loading commit variant for {rev}"))?;
    Ok(variant.data().to_vec())
}
