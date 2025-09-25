use alloc::string::{String, ToString};

pub struct Index {
    location: u64,
    permissions: u16,
    file_type: u8,
    name: String,
}

impl Index {
    pub fn new() -> Self {
        Index {
            location: 0,
            permissions: 0o644, // Default file permissions
            file_type: 0,
            name: "Null".to_string(),
        }
    }
    
    pub fn with_details(location: u64, permissions: u16, file_type: u8, name: String) -> Self {
        Index { location, permissions, file_type, name }
    }
}