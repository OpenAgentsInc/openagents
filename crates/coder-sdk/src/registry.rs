//! The hooks a session runs: by event, tool matcher, timeout, source and
//! declared disclosure.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::events::{Call, Event};
use super::outcome::{HookOutcome, Permission};

/// Who registered a hook, which sets what it may answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Host built-ins: floors, squeeze, the worktree gate. Full authority.
    BuiltIn,
    /// System One decision hooks: never allow past a rule, never rewrite.
    Decision,
    /// A plugin's Wasm guest: tighten only, rewrite only when the person
    /// grants it.
    Plugin,
    /// A project's or person's shell hook.
    Command,
}

impl Source {
    /// Whether this source's `Allow` counts. A plugin only tightens.
    #[must_use]
    pub fn may_allow(self) -> bool {
        self != Source::Plugin
    }
}

/// What a hook sends out of the session, and to whom.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Disclosure {
    /// What of the session it sends, such as `the command`.
    pub sends: String,
    /// Who receives it, such as a decision model or a plugin.
    pub to: String,
}

/// The versions and model a hook is pinned to, digested with the hook set.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Versions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// A hook's body: the call in, an outcome or an error out.
pub type Run = Arc<dyn Fn(&Call) -> Result<HookOutcome, String> + Send + Sync>;

/// The timeout a hook gets unless it names one.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// One registered hook.
#[derive(Clone)]
pub struct Hook {
    pub id: String,
    pub event: Event,
    /// Which tools it fires on: a name, names joined by `|`, or `*`. None
    /// fires on every call of the event.
    pub matcher: Option<String>,
    pub timeout: Duration,
    pub source: Source,
    /// What it sends out; none for a hook that sends nothing.
    pub disclosure: Option<Disclosure>,
    /// A floor's rule answer, used when the floor fails. A floor's answer
    /// is never overridden.
    pub floor: Option<Permission>,
    /// The hook must be able to deny before the harness acts, so a backend
    /// that only sees the event afterwards refuses it at start.
    pub must_deny: bool,
    /// A plugin's rewrite counts only when the person granted it.
    pub rewrite_granted: bool,
    pub versions: Versions,
    pub run: Run,
}

impl std::fmt::Debug for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hook")
            .field("id", &self.id)
            .field("event", &self.event)
            .field("matcher", &self.matcher)
            .field("source", &self.source)
            .field("floor", &self.floor)
            .finish_non_exhaustive()
    }
}

impl Hook {
    /// A hook on `event` from `source`, firing on every call.
    pub fn new(
        id: &str,
        event: Event,
        source: Source,
        run: impl Fn(&Call) -> Result<HookOutcome, String> + Send + Sync + 'static,
    ) -> Self {
        Hook {
            id: id.to_string(),
            event,
            matcher: None,
            timeout: DEFAULT_TIMEOUT,
            source,
            disclosure: None,
            floor: None,
            must_deny: false,
            rewrite_granted: false,
            versions: Versions::default(),
            run: Arc::new(run),
        }
    }

    /// A floor: a built-in that must deny, with `fallback` its rule's
    /// answer when it fails.
    pub fn floor(
        id: &str,
        event: Event,
        fallback: Permission,
        run: impl Fn(&Call) -> Result<HookOutcome, String> + Send + Sync + 'static,
    ) -> Self {
        Hook {
            floor: Some(fallback),
            must_deny: true,
            ..Hook::new(id, event, Source::BuiltIn, run)
        }
    }

    #[must_use]
    pub fn matching(mut self, matcher: &str) -> Self {
        self.matcher = Some(matcher.to_string());
        self
    }

    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn disclosing(mut self, sends: &str, to: &str) -> Self {
        self.disclosure = Some(Disclosure {
            sends: sends.to_string(),
            to: to.to_string(),
        });
        self
    }

    #[must_use]
    pub fn needs_deny(mut self) -> Self {
        self.must_deny = true;
        self
    }

    #[must_use]
    pub fn granted_rewrite(mut self) -> Self {
        self.rewrite_granted = true;
        self
    }

    #[must_use]
    pub fn pinned(mut self, versions: Versions) -> Self {
        self.versions = versions;
        self
    }

    #[must_use]
    pub fn is_floor(&self) -> bool {
        self.floor.is_some()
    }

    /// Whether the hook fires on a call of its event on `tool`.
    #[must_use]
    pub fn matches(&self, tool: Option<&str>) -> bool {
        match (self.matcher.as_deref(), tool) {
            (None | Some("*" | ""), _) => true,
            (Some(_), None) => false,
            (Some(matcher), Some(tool)) => matcher.split('|').any(|name| name.trim() == tool),
        }
    }

    /// Whether its rewrite counts: never on a floor (the floors judge the
    /// final input) or a decision hook, a plugin only when granted.
    #[must_use]
    pub fn may_rewrite(&self) -> bool {
        !self.is_floor()
            && match self.source {
                Source::BuiltIn | Source::Command => true,
                Source::Decision => false,
                Source::Plugin => self.rewrite_granted,
            }
    }
}

/// The hooks by event, in registration order.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    hooks: BTreeMap<Event, Vec<Hook>>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a hook after the others on its event.
    ///
    /// # Errors
    ///
    /// Refuses a second hook with the same ID on the event, and a floor
    /// whose rule answer would let the action through.
    pub fn register(&mut self, hook: Hook) -> Result<(), String> {
        if let Some(fallback) = &hook.floor
            && fallback.rank() < Permission::ask("").rank()
        {
            return Err(format!(
                "The safety rule {} must ask or refuse when it can't answer.",
                hook.id
            ));
        }
        let hooks = self.hooks.entry(hook.event).or_default();
        if hooks.iter().any(|other| other.id == hook.id) {
            return Err(format!(
                "A hook named {} is already set for {}.",
                hook.id,
                hook.event.name()
            ));
        }
        hooks.push(hook);
        Ok(())
    }

    /// The hooks on `event` that fire on `tool`, in registration order.
    pub fn matching<'a>(
        &'a self,
        event: Event,
        tool: Option<&'a str>,
    ) -> impl Iterator<Item = &'a Hook> + 'a {
        self.hooks
            .get(&event)
            .into_iter()
            .flatten()
            .filter(move |hook| hook.matches(tool))
    }

    /// Every hook, by event and then registration order.
    pub fn hooks(&self) -> impl Iterator<Item = &Hook> {
        self.hooks.values().flatten()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hooks.values().all(Vec::is_empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet(_: &Call) -> Result<HookOutcome, String> {
        Ok(HookOutcome::none())
    }

    #[test]
    fn a_matcher_picks_its_tools_and_order_is_kept() {
        let mut registry = Registry::new();
        registry
            .register(
                Hook::new("a", Event::PreToolUse, Source::BuiltIn, quiet).matching("Bash|Edit"),
            )
            .unwrap();
        registry
            .register(Hook::new("b", Event::PreToolUse, Source::Command, quiet))
            .unwrap();
        registry
            .register(Hook::new("c", Event::PreToolUse, Source::Plugin, quiet).matching("Read"))
            .unwrap();
        let ids = |tool| {
            registry
                .matching(Event::PreToolUse, tool)
                .map(|hook| hook.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(Some("Edit")), ["a", "b"]);
        assert_eq!(ids(Some("Read")), ["b", "c"]);
        assert_eq!(ids(None), ["b"]);
        assert_eq!(registry.matching(Event::Stop, None).count(), 0);
    }

    #[test]
    fn a_floor_must_fall_back_to_ask_or_deny_and_ids_are_unique() {
        let mut registry = Registry::new();
        let loose = Hook::floor("rm", Event::PreToolUse, Permission::Allow, quiet);
        assert!(registry.register(loose).is_err());
        let floor = Hook::floor("rm", Event::PreToolUse, Permission::ask("confirm"), quiet);
        assert!(floor.must_deny && floor.source == Source::BuiltIn);
        registry.register(floor.clone()).unwrap();
        assert!(registry.register(floor).is_err());
        let plugin = Hook::new("p", Event::PreToolUse, Source::Plugin, quiet)
            .disclosing("the command", "a plugin");
        assert!(!plugin.may_rewrite() && plugin.granted_rewrite().may_rewrite());
        assert!(!Source::Plugin.may_allow() && Source::Decision.may_allow());
    }
}
