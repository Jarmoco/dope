/* -----------------------------------------------------------------------------
 * Userscript File Management
 * -----------------------------------------------------------------------------
 *
 * This module provides file I/O operations for loading userscript files
 * from the filesystem. Scripts are stored with a .user.js extension in the
 * scripts/ directory.
 *
 * Architecture:
 * - read_script: Synchronously loads a userscript by name
 *
 * Design Choices:
 * - Synchronous I/O: Scripts are small files, async overhead not needed
 * - String-based path building: Simple and sufficient for this use case
 * - Result error handling: Allows caller to decide how to handle failures
 */

use std::fmt;

pub fn read_script(name: &str) -> Result<String, ScriptError> {
    let path = format!("./scripts/{}.user.js", name);
    std::fs::read_to_string(&path).map_err(|e| ScriptError { path, source: e })
}


/* -----------------------------------------------------------------------------
 * Error Types
 * -------------------------------------------------------------------------- */

#[derive(Debug)]
pub struct ScriptError {
    path: String,
    source: std::io::Error,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Error while reading userscript {}: {}",
            self.path, self.source
        )
    }
}

impl std::error::Error for ScriptError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
