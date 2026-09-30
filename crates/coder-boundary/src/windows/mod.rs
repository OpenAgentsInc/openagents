//! The Windows backend: an AppContainer per boundary, the launcher that
//! starts a command in it, and the handle-relative, no-follow opens the
//! snapshot walk and confined reads are made of.

pub(crate) mod container;
pub mod launch;
pub(crate) mod nt;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The launcher's file name, installed beside the program that builds a
/// boundary (`coder.exe`, `microcoder.exe`).
pub const LAUNCHER: &str = "coder-boundary.exe";

/// Where this process's launcher is: beside its own executable, or, for a
/// test binary Cargo keeps in `deps`, beside the build's executables one
/// directory up. It is never searched for on `PATH`.
pub fn launcher_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let directory = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        let beside = directory.join(LAUNCHER);
        let path: PathBuf = if !beside.is_file()
            && directory.file_name().is_some_and(|name| name == "deps")
            && let Some(up) = directory.parent()
        {
            up.join(LAUNCHER)
        } else {
            beside
        };
        path.to_string_lossy().into_owned()
    })
}

/// Opens the regular, singly linked file `relative` beneath `root`,
/// resolving every component relative to its parent's handle: a link
/// anywhere on the way is refused, never followed, as `openat` with
/// `O_NOFOLLOW` does on Unix.
///
/// # Errors
///
/// When a component is missing (`NotFound`), a link, not a directory, or
/// the leaf is not a singly linked regular file, or the path leaves
/// `root`.
pub fn open_beneath(root: &Path, relative: &Path) -> std::io::Result<std::fs::File> {
    nt::open_beneath(root, relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    #[test]
    fn a_confined_read_refuses_links_escapes_streams_and_hard_links() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("private"), "outside").unwrap();
        std::fs::hard_link(outside.path().join("private"), root.path().join("hard")).unwrap();
        std::fs::create_dir(root.path().join("safe")).unwrap();
        std::fs::write(root.path().join("safe").join("file"), "retained").unwrap();
        let mut refused = vec![
            r"..\outside",
            r"C:\outside",
            "hard",
            r"safe\file:stream",
            "safe",
        ];
        // Creating a link needs Developer Mode or the privilege.
        if std::os::windows::fs::symlink_dir(outside.path(), root.path().join("parent")).is_ok()
            && std::os::windows::fs::symlink_file(
                outside.path().join("private"),
                root.path().join("leaf"),
            )
            .is_ok()
        {
            refused.extend([r"parent\private", "leaf"]);
        }
        for path in refused {
            assert!(
                open_beneath(root.path(), Path::new(path)).is_err(),
                "{path}"
            );
        }
        let missing = open_beneath(root.path(), Path::new(r"safe\absent")).unwrap_err();
        assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
        let mut contents = String::new();
        open_beneath(root.path(), Path::new(r"safe\file"))
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "retained");
    }

    #[test]
    fn a_reparse_buffer_names_its_link_target_or_its_tag_and_data() {
        // A symbolic link: tag, data length, reserved, then the names'
        // offsets and lengths, the flags, and the path buffer.
        let name: Vec<u8> = r"\??\C:\target"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&0xA000_000Cu32.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&name);
        assert_eq!(nt::parse_reparse(&buffer), r"\\?\C:\target");
        let other = [0x1Au8, 0, 0, 0x90, 1, 0, 0, 0, 0xab];
        assert_eq!(nt::parse_reparse(&other), "reparse:9000001a:ab");
    }
}
