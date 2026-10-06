//! A host terminal's shell starts with the shell-integration hooks, so its
//! block journal sees each command, under a temporary `HOME`.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder_pty::ext::{BlockPageRead, BlockState};
use coder_pty::host::{Config, Host, Right, Rights, channel};
use coder_pty::wire::{Attach, Input, Launch, Mode, Open, Size, Status, Value};

const WORKSPACE: &str = "5757575757575757575757575757575757575757575757575757575757575757";
const OWNER: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";

struct Owner;

impl Rights for Owner {
    fn holds(&self, principal: &str, _: Right) -> bool {
        principal == OWNER
    }
}

fn id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

#[test]
fn a_hooked_zsh_journals_each_command() {
    let zsh = Path::new("/bin/zsh");
    if !zsh.is_file() {
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let mut config = Config::new().workspace(WORKSPACE, home.path());
    config.base_env.retain(|(name, _)| name != "HOME");
    config
        .base_env
        .push(("HOME".into(), home.path().display().to_string()));
    config.emulator = Some(coder_vt::Authority::factory(100));
    let hooks = super::terminal_shell(&mut config, Some(zsh));
    assert!(hooks.is_some(), "zsh gets hooks");
    assert_eq!(config.shell, zsh);
    let host = Host::new(config, Arc::new(Owner));
    let terminal = match host.open(
        OWNER,
        &Open::new(id(), WORKSPACE, "", Launch::Shell, Size::new(24, 80)),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => panic!("open: {other:?}"),
    };
    let (sink, _frames) = channel(1 << 16);
    let attach = Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 20).with_effects();
    host.attach(OWNER, &attach, Box::new(sink)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut typed = false;
    loop {
        let read = BlockPageRead::new(id(), terminal.clone(), None, 8);
        let Ok((_, Value::Blocks { page })) = host.block_page(OWNER, &read) else {
            panic!("the host serves block reads");
        };
        if let Some(block) = page.blocks.first()
            && block.state == BlockState::Finished
        {
            assert_eq!(block.command, "echo journaled; false");
            assert_eq!(block.status, Some(1));
            assert!(block.retained);
            break;
        }
        // Type once the shell has had time to read its startup files.
        if !typed && Instant::now() + Duration::from_secs(18) < deadline {
            host.input(
                OWNER,
                &Input::new(id(), terminal.clone(), b"echo journaled; false\n".to_vec()),
            )
            .unwrap();
            typed = true;
        }
        assert!(Instant::now() < deadline, "no finished block: {page:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(host);
    drop(hooks);
}
