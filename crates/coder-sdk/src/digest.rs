//! The hook set as part of the admission: the registered hooks, their
//! question-set and policy versions and pinned models, digested into the
//! route's adapter digest beside the delegate recipe
//! (`route_contract::recipe`).

use route_contract::Digest;
use serde::Serialize;
use serde_json::{Value, json};

use super::events::Event;
use super::registry::{Disclosure, Registry, Source, Versions};

/// The hook set document's schema.
pub const HOOK_SET_SCHEMA: &str = "openagents.coder-sdk.hook-set.v1";

/// One hook as the digest sees it: everything but its body.
#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct Entry<'a> {
    event: Event,
    id: &'a str,
    matcher: Option<&'a str>,
    source: Source,
    floor: bool,
    must_deny: bool,
    disclosure: Option<&'a Disclosure>,
    #[serde(flatten)]
    versions: &'a Versions,
}

/// The hook set document, hooks sorted by event and ID.
#[must_use]
pub fn hook_set(registry: &Registry) -> Value {
    let mut entries: Vec<Entry<'_>> = registry
        .hooks()
        .map(|hook| Entry {
            event: hook.event,
            id: &hook.id,
            matcher: hook.matcher.as_deref(),
            source: hook.source,
            floor: hook.is_floor(),
            must_deny: hook.must_deny,
            disclosure: hook.disclosure.as_ref(),
            versions: &hook.versions,
        })
        .collect();
    entries.sort();
    json!({"schema": HOOK_SET_SCHEMA, "hooks": entries})
}

/// The hook set's digest; none for a session with no hooks.
#[must_use]
pub fn hook_set_digest(registry: &Registry) -> Option<Digest> {
    (!registry.is_empty()).then(|| route_contract::digest_of(&hook_set(registry)))
}

/// The adapter digest a route records for runs on `engines` under this hook
/// set. With no hooks it is `route_contract::recipe::adapter_digest`.
#[must_use]
pub fn adapter_digest(engines: &[&str], registry: &Registry) -> Digest {
    route_contract::recipe::adapter_digest_with_hooks(engines, hook_set_digest(registry).as_ref())
}

#[cfg(test)]
mod tests {
    use super::super::outcome::{HookOutcome, Permission};
    use super::super::registry::Hook;
    use super::*;

    fn floor() -> Hook {
        Hook::floor(
            "floors",
            Event::PreToolUse,
            Permission::ask("confirm"),
            |_| Ok(HookOutcome::none()),
        )
        .pinned(Versions {
            question_set: None,
            policy: Some("floors-v1".to_string()),
            model: None,
        })
    }

    fn decision(question_set: &str) -> Hook {
        Hook::new("stuck", Event::PostToolUse, Source::Decision, |_| {
            Ok(HookOutcome::none())
        })
        .disclosing("the last output", "a decision model")
        .pinned(Versions {
            question_set: Some(question_set.to_string()),
            policy: None,
            model: Some("clef-1".to_string()),
        })
    }

    fn registry(hooks: Vec<Hook>) -> Registry {
        let mut registry = Registry::new();
        for hook in hooks {
            registry.register(hook).unwrap();
        }
        registry
    }

    #[test]
    fn no_hooks_keep_the_adapter_digest_it_had() {
        let empty = Registry::new();
        assert_eq!(hook_set_digest(&empty), None);
        assert_eq!(
            adapter_digest(&["claude"], &empty),
            route_contract::recipe::adapter_digest(&["claude"])
        );
    }

    #[test]
    fn a_version_change_or_an_added_hook_moves_the_digest() {
        let base = registry(vec![floor(), decision("stuck-v1")]);
        let digest = adapter_digest(&["claude"], &base);
        assert_ne!(digest, route_contract::recipe::adapter_digest(&["claude"]));
        // Registration order doesn't matter.
        assert_eq!(
            digest,
            adapter_digest(&["claude"], &registry(vec![decision("stuck-v1"), floor()]))
        );
        let bumped = registry(vec![floor(), decision("stuck-v2")]);
        assert_ne!(digest, adapter_digest(&["claude"], &bumped));
        let added = registry(vec![
            floor(),
            decision("stuck-v1"),
            Hook::new("squeeze", Event::PostToolUse, Source::BuiltIn, |_| {
                Ok(HookOutcome::none())
            }),
        ]);
        assert_ne!(digest, adapter_digest(&["claude"], &added));
        let document = hook_set(&base);
        assert_eq!(document["hooks"][0]["id"], "floors");
        assert_eq!(document["hooks"][1]["model"], "clef-1");
    }
}
