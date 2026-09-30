//! Owner-only files and directories on Windows.
//!
//! The host keeps its state the Unix way: a `0700` directory, `0600` files
//! opened with `O_NOFOLLOW`, and a check that each file is a regular,
//! singly linked file owned by `geteuid()` with no group or other bits.
//! Windows has none of those words, so this crate gives the same rule in
//! Windows terms:
//!
//! | Unix | Windows |
//! | --- | --- |
//! | `mkdir` with mode `0700` | [`create_dir`]: the directory, then [`restrict`] |
//! | `chmod 0600` / `0700` | [`restrict`]: owner the user, a protected DACL with one entry granting the user full control, inherited by what is created inside |
//! | owner `geteuid()`, mode `& 0o077 == 0` | [`is_private`]: the owner is the user, and every entry that applies grants only the user |
//! | `O_NOFOLLOW` | [`nofollow`]: `FILE_FLAG_OPEN_REPARSE_POINT`, so a link is opened as itself and fails the regular-file check |
//! | `st_dev`, `st_ino`, `st_nlink`, times | [`identity`]: the volume serial number, file index, link count, size, and times |
//!
//! No entry names `Administrators`, `SYSTEM`, or `Everyone`, as with the
//! control pipe. An administrator can still take ownership of anything,
//! as root can read a `0600` file; the rule is about other users.
//!
//! Wine keeps no DACL on a file, so under Wine ([`under_wine`]) only the
//! owner is checked, and the Unix mode of the file is what protects it.
//!
//! On every other platform the crate is empty, and callers keep their mode
//! bits.

#[cfg(windows)]
mod imp;

#[cfg(windows)]
pub use imp::{
    Identity, create_dir, create_dir_all, identity, identity_of, is_private, is_private_path,
    nofollow, open_dir, restrict, under_wine, user_sid,
};

/// The SDDL string of the protected, inheritable DACL [`restrict`] sets:
/// the owner `sid` and one entry granting it full control over the object
/// and everything created inside it.
#[must_use]
pub fn owner_only_sddl(sid: &str) -> String {
    format!("O:{sid}D:P(A;OICI;FA;;;{sid})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_descriptor_names_only_the_user_and_is_protected() {
        let sddl = owner_only_sddl("S-1-5-21-1-2-3-1001");
        assert_eq!(
            sddl,
            "O:S-1-5-21-1-2-3-1001D:P(A;OICI;FA;;;S-1-5-21-1-2-3-1001)"
        );
        // One allow entry, protected from inheritance, and no group names.
        assert_eq!(sddl.matches("(A;").count(), 1);
        assert!(sddl.contains("D:P("));
        for other in ["BA", "SY", "WD", "AU", "CO"] {
            assert!(!sddl.contains(&format!(";;;{other})")), "{other}");
        }
    }
}
