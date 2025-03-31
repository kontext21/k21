// use std::path::PathBuf;
// use anyhow::Result;


// fn ensure_path_exists(path: PathBuf) -> Result<PathBuf> {
//     if path.exists() {
//         Ok(path)
//     } else {
//         Err(anyhow::anyhow!("Path does not exist: {}", path.display()))
//     }
// }

// pub fn to_verified_path(path: &str) -> Result<PathBuf> {
//     let absolute_path = to_absolute_path(path)?;
//     ensure_path_exists(absolute_path)
// }

// fn to_absolute_path(path: &str) -> Result<PathBuf> {
//     let path_buf = PathBuf::from(path);

//     // if path_buf.is_file() {
//     //     return Err(anyhow::anyhow!("Path is a file, expected a directory: {}", path_buf.display()));
//     // }
    
//     if path_buf.is_absolute() {
//         return Ok(path_buf);
//     }

//     if path_buf.is_dir() {
//         match std::env::current_dir() {
//             Ok(current_dir) => {
//                 return Ok(current_dir.join(path_buf));
//             }
//             Err(e) => {
//                 return Err(anyhow::anyhow!("Failed to get current directory: {}", e));
//             }
//         }
//     }

//     let has_parent_refs = path.contains("../") || path.contains("..\\") || path == ".." || path.ends_with("/..");

//     // Convert relative path to absolute
//     match std::env::current_dir() {
//         Ok(current_dir) => {
//             let absolute_path = if has_parent_refs {
//                 // Use canonicalize to resolve parent directory references
//                 match current_dir.join(&path_buf).canonicalize() {
//                     Ok(canonical_path) => canonical_path,
//                     Err(e) => {
//                         log::warn!("Failed to canonicalize path with parent refs: {}, using simple join", e);
//                         current_dir.join(path_buf)
//                     }
//                 }
//             } else {
//                 // Simple join for paths without parent references
//                 current_dir.join(path_buf)
//             };
//             Ok(absolute_path)
//         },
//         Err(e) => {
//             log::warn!("Failed to get current directory: {}, using path as is", e);
//             Ok(path_buf)
//         }
//     }
// }

// /// Converts a string path to a PathBuf, handling absolute, relative, and canonical paths.
// /// 
// /// # Arguments
// /// * `path` - A string path that can be:
// ///   - Absolute path ("/Users/.../file")
// ///   - Relative path ("./file")
// ///   - Path with parent references ("../file")
// /// 
// /// # Returns
// /// * `Result<PathBuf>` - The converted path or an error
// /// 
// /// # Examples
// /// ```
// /// use k21::common::path_utils::parse_path;
// /// 
// /// let path = parse_path("/absolute/path/file.txt").unwrap();
// /// let relative = parse_path("./relative/file.txt").unwrap();
// /// let parent = parse_path("../parent/file.txt").unwrap();
// /// ```
// pub fn parse_path(path: &str) -> Result<PathBuf> {
//     let path_buf = PathBuf::from(path);
    
//     // If it's already absolute, return it
//     if path_buf.is_absolute() {
//         return Ok(path_buf);
//     }

//     // Get current directory
//     let current_dir = std::env::current_dir()
//         .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;

//     // Check if path contains parent directory references
//     let has_parent_refs = path.contains("../") || 
//                          path.contains("..\\") || 
//                          path == ".." || 
//                          path.ends_with("/..");

//     // Convert to absolute path
//     if has_parent_refs {
//         // Use canonicalize for paths with parent references
//         current_dir.join(&path_buf)
//             .canonicalize()
//             .map_err(|e| anyhow::anyhow!("Failed to resolve path with parent references: {}", e))
//     } else {
//         // Simple join for regular relative paths
//         Ok(current_dir.join(path_buf))
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::env;

//     #[test]
//     fn test_absolute_path() {
//         let abs_path = if cfg!(windows) {
//             "C:\\Users\\test\\file.txt"
//         } else {
//             "/Users/test/file.txt"
//         };
//         let result = parse_path(abs_path);
//         assert!(result.is_ok());
//         assert!(result.unwrap().is_absolute());
//     }

//     #[test]
//     fn test_relative_path() {
//         let result = parse_path("./test.txt");
//         assert!(result.is_ok());
//         assert!(result.unwrap().is_absolute());
//     }

//     #[test]
//     fn test_parent_path() {
//         let current_dir = env::current_dir().unwrap();
//         if let Some(parent) = current_dir.parent() {
//             let result = parse_path("../test.txt");
//             assert!(result.is_ok());
//             assert!(result.unwrap().starts_with(parent));
//         }
//     }
// }



