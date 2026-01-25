pub mod ipc;

use std::path::Path;

pub fn export_ts(path: &Path) -> Result<(), std::io::Error> {
    ipc::export_ts(path)
}
