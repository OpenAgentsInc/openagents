//! Coder's hook sets on the Coder SDK (`docs/coder/design/coder-sdk.md`):
//! the same built-ins registered once and run by whichever backend takes
//! the turn, Claude Code (#11496) or Coder's own loop (#11497).

use std::path::Path;

use coder_sdk::{Budget, Registry};

/// The built-in hooks a session runs: the floors on every turn, and on a
/// full turn the token savers' squeezer, registered before them in the
/// rewrite chain (the floors judge the final input whatever the order).
#[must_use]
pub fn registry(cwd: &Path, full: bool) -> Registry {
    let mut registry = Registry::new();
    let mut hooks = vec![crate::floors::sdk_hook(cwd.to_owned())];
    if full {
        hooks.extend(crate::token_savers::sdk_hooks());
    }
    for hook in hooks {
        // Built-in IDs are distinct, and the floors fall back to ask.
        let _ = registry.register(hook);
    }
    registry
}

/// The Claude Code backend for a session in `cwd`: [`registry`], with the
/// floors' asks answered on the approval card where someone can answer.
#[must_use]
pub fn claude(cwd: &Path, full: bool) -> coder_sdk::claude::Claude {
    coder_sdk::claude::Claude::new(registry(cwd, full), Budget::default())
        .expect("every hook event is native on Claude Code")
        .asking(crate::floors::asker(
            cwd.to_owned(),
            crate::ui::claude_rail_label,
        ))
}
