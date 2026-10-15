//! Portable session descriptions without linking execution code into clients.

pub use route_contract::session::{
    Backend, Controls, Descriptor, HookSupport, NativeSupport, SCHEMA, Selection,
};

use crate::{HookBackend, HookTier, hook_tiers};

/// Describe the selected adapter's hook support. Native operations are supplied
/// explicitly by the caller; hook tiers never imply lifecycle or media support.
pub fn backend(
    selection: Selection,
    adapter: &str,
    hooks: Option<HookBackend>,
    native: NativeSupport,
) -> Backend {
    Backend {
        selection,
        adapter: adapter.into(),
        native,
        hooks: hooks
            .into_iter()
            .flat_map(hook_tiers)
            .map(|(event, tier)| {
                let support = match tier {
                    HookTier::Native => HookSupport::Native,
                    HookTier::Command => HookSupport::Command,
                    HookTier::Emulated => HookSupport::Emulated,
                    HookTier::None => HookSupport::Unsupported,
                };
                (event.into(), support)
            })
            .collect(),
        ..Backend::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_backend_does_not_grant_native_controls() {
        let described = backend(
            Selection::default(),
            "codex-app-server",
            Some(HookBackend::CodexAppServer),
            NativeSupport::default(),
        );
        assert_eq!(described.hooks["PreToolUse"], HookSupport::Native);
        assert_eq!(described.native, NativeSupport::default());
        let unknown = backend(
            Selection::default(),
            "unknown",
            None,
            NativeSupport::default(),
        );
        assert!(unknown.hooks.is_empty());
    }
}
