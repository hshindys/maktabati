//! `maktabati-files` — the filesystem half of the attachment contract.
//!
//! [`maktabati_domain::BookFile`] owns the row's invariants and knows where its bytes live;
//! this crate owns the bytes themselves: reading them to hash, and laying them down.
//!
//! ## Why content-addressing
//!
//! In copy mode the blob is stored at `<data_dir>/files/<sha256>` — the name *is* the content.
//! That buys three things for free:
//!
//! * the same file added to two books is stored once (`ux_files_sha_copy` in
//!   `0001_init.sql` is the database's half of the same rule);
//! * a file that was moved or edited no longer matches its row, which is detectable rather than
//!   silent;
//! * a partially written blob can be discarded and retried, because no name collision is
//!   possible.

#![forbid(unsafe_code)]

use std::path::Path;

use sha2::{Digest, Sha256};

pub use maktabati_domain::file::limits;

/// Directory, under the app's data directory, that owns copied attachments.
pub use maktabati_domain::file::limits::FILES_SUBDIR;

/// SHA-256 of a byte stream as lower-case hex.
///
/// Written here rather than in `storage` so that the one place that decides *how* a file is
/// named also decides how it is hashed: the name is the hash, so the two cannot drift apart.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Read a file and return its bytes together with their SHA-256.
///
/// # Errors
///
/// * [`std::io::Error`] when the file cannot be opened or read — the caller maps this onto the
///   storage error surface so the webview sees one vocabulary.
pub fn read_and_hash(path: &Path) -> std::io::Result<(Vec<u8>, String)> {
    let bytes = std::fs::read(path)?;
    let hash = sha256_hex(&bytes);
    Ok((bytes, hash))
}

/// Hash a file by streaming it, returning its size in bytes alongside the digest.
///
/// Streaming is the point: an attachment can be a gigabyte of audio, and reading it whole to
/// hash it would put that in memory at once for no benefit. The size comes from the same pass,
/// so the caller does not need a second `metadata` call that could disagree with the bytes it
/// actually hashed.
///
/// # Errors
///
/// * [`std::io::Error`] when the file cannot be opened or read.
pub fn hash_file(path: &Path) -> std::io::Result<(u64, String)> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
        hasher.update(&buf[..n]);
    }

    let hash = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok((total, hash))
}

/// Absolute path of the blob that owns `sha256` in copy mode.
#[must_use]
pub fn blob_path(data_dir: &Path, sha256: &str) -> std::path::PathBuf {
    data_dir.join(FILES_SUBDIR).join(sha256)
}

/// `true` when a blob for `sha256` already sits in the library.
#[must_use]
pub fn blob_exists(data_dir: &Path, sha256: &str) -> bool {
    blob_path(data_dir, sha256).is_file()
}

/// Copy `source` into the library as the blob named `sha256`, creating the directory if needed.
///
/// A no-op when the blob is already there: the destination name is derived from the content, so
/// an existing file with the same name is by definition the same content.
///
/// # Errors
///
/// * [`std::io::Error`] when the destination directory cannot be created or the copy fails.
pub fn store_blob(data_dir: &Path, sha256: &str, source: &Path) -> std::io::Result<()> {
    let dest = blob_path(data_dir, sha256);
    if dest.is_file() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, &dest)?;
    Ok(())
}

/// Remove a blob, ignoring a missing file.
///
/// # Errors
///
/// * [`std::io::Error`] when the removal fails for a reason other than absence.
pub fn remove_blob(data_dir: &Path, sha256: &str) -> std::io::Result<()> {
    let dest = blob_path(data_dir, sha256);
    match std::fs::remove_file(dest) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_lower_case_hex_and_deterministic() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), sha256_hex(b"abc"));
        assert_ne!(sha256_hex(b"abc"), sha256_hex(b"abd"));
    }

    #[test]
    fn blob_path_is_content_named() {
        let p = blob_path(Path::new("/tmp/lib"), "deadbeef");
        assert_eq!(p, Path::new("/tmp/lib/files/deadbeef"));
    }

    #[test]
    fn store_then_remove_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("src.epub");
        std::fs::write(&source, b"book-bytes").unwrap();

        let (size, hash) = hash_file(&source).unwrap();
        assert_eq!(size, 10);

        let data = dir.path().join("data");
        assert!(!blob_exists(&data, &hash));
        store_blob(&data, &hash, &source).unwrap();
        assert!(blob_exists(&data, &hash));

        // Storing the same content again is a no-op, not an error.
        store_blob(&data, &hash, &source).unwrap();
        assert_eq!(read_and_hash(&blob_path(&data, &hash)).unwrap().0, b"book-bytes");

        remove_blob(&data, &hash).unwrap();
        assert!(!blob_exists(&data, &hash));
        // Removing twice is fine.
        remove_blob(&data, &hash).unwrap();
    }

    #[test]
    fn hashing_a_large_file_agrees_with_reading_it_whole() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("big.bin");
        let chunk = vec![0xABu8; 1024 * 1024];
        let mut handle = std::fs::File::create(&source).unwrap();
        use std::io::Write;
        for _ in 0..8 {
            handle.write_all(&chunk).unwrap();
        }
        drop(handle);

        let (size, streamed) = hash_file(&source).unwrap();
        assert_eq!(size, 8 * 1024 * 1024);
        assert_eq!(streamed.len(), 64);
        // The streamed pass must agree with the whole-file pass on the same bytes.
        assert_eq!(streamed, read_and_hash(&source).unwrap().1);
    }

    #[test]
    fn reading_a_missing_file_is_an_error() {
        assert!(hash_file(Path::new("/definitely/not/here.epub")).is_err());
    }
}
