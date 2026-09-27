//! The fake `ssh` harness shared by `coder-ssh` and its clients' tests.
//!
//! [`shim`] writes a program that stands in for `ssh`: it records each
//! argument list in `DIR/calls`, answers `ssh -G`, asks for a password
//! through `SSH_ASKPASS` when `DIR/password` exists, refuses port forwarding
//! when `DIR/no-tunnel` exists, records each tunnel's process and forwarding
//! in `DIR/tunnel` and then sleeps without forwarding, and runs the remote
//! command in a local shell with `HOME` set to a temporary "remote" home.
//! [`archive`] packs a stand-in `coder` program as a release archive. A
//! test that includes this file never reads or writes the real `~/.ssh` or
//! `~/.openagents`. Include it with
//! `#[path = "../../coder-ssh/tests/support/fake_ssh.rs"]`.
#![allow(dead_code)]

use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest as _, Sha256};

/// Write the fake `ssh` program at `dir/ssh`. It runs remote commands with
/// `shell` and `HOME=home`, and puts `dir/bin` first on `PATH`.
pub fn shim(dir: &Path, home: &Path, shell: &str) -> PathBuf {
    let d = dir.display();
    let home = home.display().to_string();
    let shim = format!(
        r#"#!/bin/sh
# Fake ssh for Coder SSH tests.
printf '%s\n' "$*" >> '{d}/calls'
resolve=no
tunnel=no
forward=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -G) resolve=yes; shift ;;
    -N) tunnel=yes; shift ;;
    -T) shift ;;
    -o) shift 2 ;;
    -L) forward=$2; shift 2 ;;
    --) shift; break ;;
    -*) echo "fake ssh: unexpected option $1" >&2; exit 255 ;;
    *) break ;;
  esac
done
shift
if [ "$resolve" = yes ]; then
  printf 'hostname fake.example\nuser fake\nport 2222\ncontrolmaster false\n'
  exit 0
fi
if [ -f '{d}/password' ]; then
  if [ "${{SSH_ASKPASS_REQUIRE:-}}" != force ]; then echo 'Permission denied (batch).' >&2; exit 255; fi
  printf '%s\n' "$SSH_ASKPASS" >> '{d}/askpass-paths'
  if env | grep -F -q "$(cat '{d}/password')"; then echo 'password in environment' >&2; exit 254; fi
  answer=$("$SSH_ASKPASS" "fake@fake.example's password: ") || {{ echo 'Permission denied.' >&2; exit 255; }}
  if [ "$answer" != "$(cat '{d}/password')" ]; then echo 'Permission denied.' >&2; exit 255; fi
fi
if [ "$tunnel" = yes ] && [ -f '{d}/no-tunnel' ]; then
  echo 'fake ssh: port forwarding refused' >&2; exit 255
fi
if [ "$tunnel" = yes ]; then
  printf '%s %s\n' "$$" "$forward" > '{d}/tunnel'
  exec sleep 600
fi
if [ -f '{d}/corrupt' ]; then
  case "$*" in
    *oa-ssh-upload*)
      {{ cat; printf 'x'; }} | env HOME='{home}' PATH='{d}/bin':"$PATH" {shell} -c "$*"
      exit $? ;;
  esac
fi
exec env HOME='{home}' PATH='{d}/bin':"$PATH" {shell} -c "$*"
"#
    );
    let path = dir.join("ssh");
    std::fs::write(&path, shim).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Pack `coder` (a program's text) as the only file of a gzip-compressed tar
/// archive at `dir/name`.
pub fn archive(dir: &Path, coder: &str, name: &str) -> PathBuf {
    let bundle = dir.join(format!("bundle-{name}"));
    std::fs::create_dir_all(&bundle).unwrap();
    let binary = bundle.join("coder");
    std::fs::write(&binary, coder).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let archive = dir.join(name);
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&bundle)
        .arg("coder")
        .status()
        .unwrap();
    assert!(status.success());
    archive
}

/// The lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> String {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
