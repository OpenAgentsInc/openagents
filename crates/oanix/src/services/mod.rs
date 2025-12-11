//! FileService implementations
//!
//! Concrete implementations of the FileService trait for use in OANIX namespaces.
//!
//! ## Available Filesystems
//!
//! - [`MemFs`] - In-memory read/write filesystem
//! - [`MapFs`] - Static/immutable filesystem from embedded data
//! - [`FuncFs`] - Dynamic files computed via closures
//! - [`CowFs`] - Copy-on-Write overlay for snapshots

mod cow_fs;
mod func_fs;
mod map_fs;
mod mem_fs;

pub use cow_fs::CowFs;
pub use func_fs::{FuncFs, ReadFn, WriteFn};
pub use map_fs::{MapFs, MapFsBuilder};
pub use mem_fs::MemFs;
