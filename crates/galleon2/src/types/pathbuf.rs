//! Custom PathBuf type for nostd environments.

use alloc::string::String;

pub struct PathBuf {
    pub path: String,
}

impl PathBuf {
    /// Creates a new, empty `PathBuf`.
    pub fn new(path: String) -> Self {
        PathBuf { path }
    }

    /// Appends a path segment to the `PathBuf`.
    pub fn push(&mut self, segment: &str) {
        if !self.path.ends_with('/') && !self.path.is_empty() {
            self.path.push('/');
        }
        self.path.push_str(segment);
    }

    /// Returns the string representation of the path.
    pub fn as_str(&self) -> &str {
        &self.path
    }
}
