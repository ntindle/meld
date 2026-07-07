use crate::errors::Result;
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// SHA-256 of a file's contents, streamed in 64 KiB chunks.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
