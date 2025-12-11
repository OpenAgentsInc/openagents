//! FileService implementations
//!
//! Concrete implementations of the FileService trait for use in OANIX namespaces.

mod mem_fs;

pub use mem_fs::MemFs;
