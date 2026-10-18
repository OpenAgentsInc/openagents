//! Host-owned caller identities for run commands. Permissions live in account grants.
use coder_access::Code;
use std::{collections::BTreeMap, sync::RwLock};

/// The caller a host operator bound to one signed native device grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Principal {
    /// The owner acting through an explicitly selected device grant.
    Owner,
    /// An agent definition; its account grants still decide each action.
    Agent(String),
}
/// Identity bindings only. This stores no command permissions. Unbound native
/// grants cannot call run controls; the host operator restores bindings on restart.
#[derive(Default)]
pub struct Principals(RwLock<BTreeMap<String, Principal>>);
impl Principals {
    /// Bind a native grant to an owner or definition. Remote callers cannot edit it.
    pub fn bind(&self, native_grant: &str, principal: Principal) -> Result<(), Code> {
        let valid = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if !valid(native_grant) || matches!(&principal, Principal::Agent(id) if !valid(id)) {
            return Err(Code::Malformed);
        }
        let mut book = self.0.write().map_err(|_| Code::Unavailable)?;
        if book.len() >= 8192 && !book.contains_key(native_grant) {
            return Err(Code::Bounds);
        }
        book.insert(native_grant.into(), principal);
        Ok(())
    }
    /// Remove a native grant's identity binding; subsequent controls refuse.
    pub fn unbind(&self, native_grant: &str) -> Result<(), Code> {
        self.0
            .write()
            .map_err(|_| Code::Unavailable)?
            .remove(native_grant);
        Ok(())
    }
    /// Resolve the current host-owned identity, never a caller-provided definition.
    pub fn resolve(&self, native_grant: &str) -> Result<Principal, Code> {
        self.0
            .read()
            .map_err(|_| Code::Unavailable)?
            .get(native_grant)
            .cloned()
            .ok_or(Code::Forbidden)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_identity_bindings_default_deny_and_cannot_use_agent_names() {
        let book = Principals::default();
        let native = "b".repeat(64);
        let agent = "a".repeat(64);
        assert_eq!(book.resolve(&native), Err(Code::Forbidden));
        assert_eq!(
            book.bind(&native, Principal::Agent("a name".into())),
            Err(Code::Malformed)
        );
        book.bind(&native, Principal::Agent(agent.clone())).unwrap();
        assert_eq!(book.resolve(&native), Ok(Principal::Agent(agent)));
        book.unbind(&native).unwrap();
        assert_eq!(book.resolve(&native), Err(Code::Forbidden));
        book.bind(&native, Principal::Owner).unwrap();
        assert_eq!(book.resolve(&native), Ok(Principal::Owner));
    }
}
