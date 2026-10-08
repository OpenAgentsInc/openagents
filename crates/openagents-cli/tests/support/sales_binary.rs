//! Freeze installed acceptance to a copied executable when the gate requests it.
use sha2::{Digest, Sha256};
use std::path::PathBuf;
pub fn path() -> PathBuf {
    let Some(path) = std::env::var_os("OPENAGENTS_TEST_CLI_BINARY") else {
        return env!("CARGO_BIN_EXE_openagents").into();
    };
    let path = PathBuf::from(path);
    assert!(
        path.is_absolute(),
        "copied acceptance binary must have an absolute path"
    );
    let metadata = std::fs::symlink_metadata(&path).expect("copied acceptance binary unavailable");
    assert!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "copied acceptance binary must be a regular file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            metadata.permissions().mode() & 0o077,
            0,
            "copied acceptance binary must be private"
        );
    }
    let expected = std::env::var("OPENAGENTS_TEST_CLI_SHA256")
        .expect("copied acceptance binary digest unavailable");
    let actual =
        Sha256::digest(std::fs::read(&path).expect("copied acceptance binary read failed"))
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
    assert_eq!(actual, expected, "copied acceptance binary changed");
    path
}
