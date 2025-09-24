//! MFT (Master File Table) is the what michealsoft uses to index their files
//! it stores the location, permissions, file type and name.
//! Sadly we cant make this really close to how michealsoft does it do to copyright or something.
//! MFT is faster than inodes which is why microsoft uses it
//! either that or its because linux uses inodes in their fs.

use alloc::string::String;

pub struct MFT {
    location: u64,
    permissions: u8,
    file_type: u8,
    name: String,
}