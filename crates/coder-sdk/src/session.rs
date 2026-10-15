//! Portable session descriptions without linking execution code into clients.

pub use route_contract::session::control;

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

/// Overrides the exercised adapter can apply without guessing provider options.
pub fn efforts(adapter: &str) -> &'static [&'static str] {
    match adapter {
        "claude-sdk" => &["low", "medium", "high", "xhigh"],
        _ => &[],
    }
}

/// Apply a host-validated start through Claude's existing SDK session options.
/// Continue preserves the adapter's existing resume choice; fresh and explicit
/// native resume have separate behavior and never substitute for each other.
pub fn claude_start(
    start: &control::Start,
    mut options: claude_agent_sdk::QueryOptions,
) -> Result<claude_agent_sdk::QueryOptions, String> {
    use claude_agent_sdk::EffortLevel;
    if start.selection.provider != "claude"
        || start
            .request
            .as_ref()
            .is_some_and(|request| request.adapter != "claude-sdk")
        || !start.valid_for(
            "claude",
            &start.selection.model,
            start.selection.effort.as_deref(),
        )
    {
        return Err("This Claude session configuration is not supported.".into());
    }
    options.model = Some(start.selection.model.clone());
    if let Some(effort) = start.selection.effort.as_deref() {
        options.effort = Some(match effort {
            "low" => EffortLevel::Low,
            "medium" => EffortLevel::Medium,
            "high" => EffortLevel::High,
            "xhigh" => EffortLevel::Xhigh,
            _ => return Err("Claude Code does not support this reasoning choice.".into()),
        });
    }
    match start.mode() {
        control::Mode::New | control::Mode::ContextTransfer => options.resume = None,
        control::Mode::NativeResume => {
            options.resume = Some(
                start
                    .native
                    .as_ref()
                    .ok_or("The saved session is unavailable.")?
                    .session
                    .clone(),
            );
        }
        control::Mode::Continue => {}
    }
    Ok(options)
}

/// Add only the context the owner explicitly selected for a fresh conversation.
pub fn context_prompt(start: &control::Start, prompt: &str) -> Result<String, String> {
    match (&start.context, start.mode()) {
        (Some(context), control::Mode::ContextTransfer) if context.valid() => Ok(format!(
            "Earlier task context:\n{}\n\nNew message:\n{}",
            context.text, prompt
        )),
        (None, mode) if mode != control::Mode::ContextTransfer => Ok(prompt.into()),
        _ => Err("The earlier task summary is unavailable. Start a new session instead.".into()),
    }
}

#[cfg(test)]
mod start_tests {
    use super::*;
    fn start(mode: control::Mode) -> control::Start {
        let selection = Selection {
            provider: "claude".into(),
            model: "fixture".into(),
            effort: Some("high".into()),
        };
        control::Start {
            request: Some(control::Request {
                schema: control::SCHEMA.into(),
                offer: route_contract::digest_of(&()),
                adapter: "claude-sdk".into(),
                checkout: "/fixture".into(),
                target: "inode:1".into(),
                selection: selection.clone(),
                mode,
                resume: None,
            }),
            selection,
            account: Some("login".into()),
            task_model: None,
            context: None,
            native: None,
        }
    }
    #[test]
    fn fresh_continue_and_native_resume_have_distinct_options() {
        let options = || claude_agent_sdk::QueryOptions {
            resume: Some("legacy".into()),
            ..Default::default()
        };
        assert_eq!(
            claude_start(&start(control::Mode::Continue), options())
                .unwrap()
                .resume
                .as_deref(),
            Some("legacy")
        );
        assert!(
            claude_start(&start(control::Mode::New), options())
                .unwrap()
                .resume
                .is_none()
        );
        let mut source = start(control::Mode::NativeResume);
        let native = control::Native {
            schema: control::NATIVE_SCHEMA.into(),
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            session: "exact-provider-id".into(),
            provider: "claude".into(),
            model: "fixture".into(),
            adapter: "claude-sdk".into(),
            checkout: "/fixture".into(),
            target: "inode:1".into(),
            account: source.account.clone(),
        };
        source.request.as_mut().unwrap().resume = Some(native.id());
        source.native = Some(native);
        assert_eq!(
            claude_start(&source, options()).unwrap().resume.as_deref(),
            Some("exact-provider-id")
        );
        source.native = None;
        assert!(claude_start(&source, options()).is_err());
    }
    #[test]
    fn unsupported_choices_fail_and_context_is_only_added_explicitly() {
        let mut source = start(control::Mode::ContextTransfer);
        assert!(context_prompt(&source, "message").is_err());
        source.context = Some(control::Context {
            task: "task".into(),
            turn: 1,
            text: "summary".into(),
        });
        assert_eq!(
            context_prompt(&source, "message").unwrap(),
            "Earlier task context:\nsummary\n\nNew message:\nmessage"
        );
        assert!(
            claude_start(&source, Default::default())
                .unwrap()
                .resume
                .is_none()
        );
        source.selection.effort = Some("invented".into());
        source.request.as_mut().unwrap().selection = source.selection.clone();
        assert!(claude_start(&source, Default::default()).is_err());
        let mut source = start(control::Mode::New);
        source.request.as_mut().unwrap().adapter = "claude-cli".into();
        assert!(claude_start(&source, Default::default()).is_err());
        assert_eq!(
            context_prompt(&start(control::Mode::Continue), "message").unwrap(),
            "message"
        );
    }
}
