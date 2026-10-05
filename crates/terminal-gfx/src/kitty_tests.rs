//! The Kitty keyboard protocol against Neovim, which negotiates it (#10691).

use super::pty::Program;
use super::{KeyIn, Overlay};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use winit::keyboard::{Key as Logical, KeyCode, ModifiersState, NamedKey, SmolStr};

fn wait(overlay: &mut Overlay, what: &str, done: impl Fn(&Overlay) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !done(overlay) {
        assert!(
            Instant::now() < deadline,
            "{what}: {:?}",
            overlay.focused_text()
        );
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn key(code: KeyCode, logical: Logical, text: Option<&str>) -> KeyIn {
    KeyIn {
        code,
        logical,
        text: text.map(str::to_owned),
        plain: text.map(str::to_owned),
        pressed: true,
        repeat: false,
        synthetic: false,
    }
}

#[test]
fn neovim_negotiates_disambiguation_and_tells_ctrl_i_from_tab() {
    let Some(nvim) = std::env::var_os("OPENAGENTS_TEST_NVIM")
        .map(PathBuf::from)
        .or_else(|| {
            [
                "/opt/homebrew/bin/nvim",
                "/usr/local/bin/nvim",
                "/usr/bin/nvim",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|path| path.is_file())
        })
    else {
        eprintln!("no Neovim installed; set OPENAGENTS_TEST_NVIM to run this check");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let program = Program::Command {
        program: nvim,
        args: [
            "--clean",
            "-i",
            "NONE",
            "-c",
            "nnoremap <C-i> :echo 'pressed CTRL-I'<CR>",
            "-c",
            "nnoremap <Tab> :echo 'pressed TAB'<CR>",
        ]
        .map(str::to_owned)
        .to_vec(),
        label: "nvim".into(),
    };
    let mut overlay = Overlay::with(home.path(), PathBuf::from("/bin/sh"), program);
    overlay.core.paper.on = false;
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    wait(&mut overlay, "Neovim did not push the flags", |overlay| {
        overlay
            .panes
            .values()
            .any(|pane| pane.session.vt.kitty_flags() == coder_vt::input::kitty::DISAMBIGUATE)
    });
    // Ctrl+I sends its own CSI u sequence, not Tab's byte.
    overlay.modifiers(ModifiersState::CONTROL);
    let ctrl_i = key(
        KeyCode::KeyI,
        Logical::Character(SmolStr::new("i")),
        Some("i"),
    );
    assert_eq!(
        overlay.core.encode(&super::input(&ctrl_i)).as_deref(),
        Some(&b"\x1b[105;5u"[..])
    );
    overlay.key(&ctrl_i);
    overlay.modifiers(ModifiersState::empty());
    wait(&mut overlay, "Neovim did not see Ctrl+I", |overlay| {
        overlay
            .focused_text()
            .is_some_and(|text| text.contains("pressed CTRL-I"))
    });
    let tab = key(KeyCode::Tab, Logical::Named(NamedKey::Tab), Some("\t"));
    assert_eq!(
        overlay.core.encode(&super::input(&tab)).as_deref(),
        Some(&b"\t"[..])
    );
    overlay.key(&tab);
    wait(&mut overlay, "Neovim did not see Tab", |overlay| {
        overlay
            .focused_text()
            .is_some_and(|text| text.contains("pressed TAB"))
    });
}
