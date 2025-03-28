use std::path::PathBuf;
use anyhow::Result;


pub fn ensure_path_exists(path: PathBuf) -> Result<PathBuf> {
    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!("Path does not exist: {}", path.display()))
    }
}

pub fn to_verified_path(path: &str) -> Result<PathBuf> {
    let absolute_path = to_absolute_path(path)?;
    ensure_path_exists(absolute_path)
}

pub fn to_absolute_path(path: &str) -> Result<PathBuf> {
    let path_buf = PathBuf::from(path);

    if path_buf.is_file() {
        return Err(anyhow::anyhow!("Path is a file, expected a directory: {}", path_buf.display()));
    }
    
    if path_buf.is_absolute() {
        return Ok(path_buf);
    }

    if path_buf.is_dir() {
        match std::env::current_dir() {
            Ok(current_dir) => {
                return Ok(current_dir.join(path_buf));
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to get current directory: {}", e));
            }
        }
    }

    let has_parent_refs = path.contains("../") || path.contains("..\\") || path == ".." || path.ends_with("/..");

    // Convert relative path to absolute
    match std::env::current_dir() {
        Ok(current_dir) => {
            let absolute_path = if has_parent_refs {
                // Use canonicalize to resolve parent directory references
                match current_dir.join(&path_buf).canonicalize() {
                    Ok(canonical_path) => canonical_path,
                    Err(e) => {
                        log::warn!("Failed to canonicalize path with parent refs: {}, using simple join", e);
                        current_dir.join(path_buf)
                    }
                }
            } else {
                // Simple join for paths without parent references
                current_dir.join(path_buf)
            };
            Ok(absolute_path)
        },
        Err(e) => {
            log::warn!("Failed to get current directory: {}, using path as is", e);
            Ok(path_buf)
        }
    }
}
