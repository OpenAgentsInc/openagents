//! Provider bindings. A provider owns resource lifetime and reports what it
//! actually observed; it never decides lease authority.
use crate::Result;
use serde::{Deserialize, Serialize};

/// A realized resource. Ownership evidence is exact provider-issued bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Realized {
    /// Opaque provider resource ID.
    pub resource: String,
    /// Opaque exact provider generation.
    pub generation: String,
    /// Host logical account the resource belongs to.
    pub account: String,
    /// Provider ownership evidence bytes, validated by this binding.
    pub ownership: Vec<u8>,
}

/// Cost evidence. Measured, declared, estimated, and unknown stay distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cost {
    Measured(u64),
    Declared(u64),
    Estimated(u64),
    Unknown,
}

/// The outcome of one provider create under an exact idempotency key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Create {
    Created(Realized, Cost),
    /// No effect occurred; the binding cannot honor this request.
    Refused(String),
    /// The response was lost or ambiguous. Reconcile; never create again.
    Unknown(String),
}

/// An exact reconciliation lookup by idempotency key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    Found(Realized),
    /// Authoritatively absent. An empty list that cannot prove absence is
    /// [`Lookup::Unknown`], not this.
    Absent,
    Unknown(String),
}

/// The outcome of destroy or detach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Teardown {
    /// The provider confirmed it, with exact receipt bytes.
    Confirmed(Vec<u8>, Cost),
    Failed(String),
    /// A timeout or lost response. Never treated as released.
    Unknown(String),
}

/// One environment provider binding.
pub trait Provider {
    /// Provider binding path. The allocator qualifies it with its own key to
    /// form the resource descriptor's provider DefinitionRef `id`.
    fn binding(&self) -> String;
    /// Create one resource under `key`. Repeating a key must not create a
    /// second resource; the allocator still never calls this twice per lease.
    fn create(&mut self, account: &str, key: &str) -> Create;
    /// Reconcile a create whose response was lost.
    fn lookup(&mut self, account: &str, key: &str) -> Lookup;
    /// Validate ownership evidence against the current resource generation.
    fn adopt(
        &mut self,
        account: &str,
        resource: &str,
        generation: &str,
        ownership: &[u8],
    ) -> Result<Realized>;
    /// Place exact component bytes and report the digests actually observed
    /// in the resource afterwards.
    fn materialize(
        &mut self,
        resource: &Realized,
        components: &[(String, Vec<u8>)],
    ) -> Result<Vec<(String, String)>>;
    /// Whether this binding implements the fixed native manifest initializer.
    fn manifest_bootstrap(&self) -> bool {
        false
    }
    /// Observe already placed components without writing them again.
    fn observe_components(&mut self, _: &Realized, _: &[String]) -> Result<Vec<(String, String)>> {
        Err("unsupported_feature: component observation is not configured".into())
    }
    /// Execute exactly once; an interrupted call is recovered only by lookup.
    fn bootstrap(&mut self, _: &Realized, _: &str, _: &serde_json::Value) -> Result<Vec<u8>> {
        Err("unsupported_feature: native bootstrap is not configured".into())
    }
    /// Read the original initialization receipt. Missing evidence is not success.
    fn bootstrap_result(&mut self, _: &Realized, _: &str) -> Result<Option<Vec<u8>>> {
        Err("unavailable: native bootstrap receipt cannot be observed".into())
    }
    /// Collect retained outputs before teardown. The allocator copies these
    /// bytes outside the resource before it may destroy it.
    fn collect(&mut self, resource: &Realized) -> Result<Vec<(String, Vec<u8>)>>;
    /// Destroy a created resource by its exact identity and generation.
    fn destroy(&mut self, resource: &Realized) -> Teardown;
    /// Observe the original teardown without repeating it. Missing receipts do
    /// not authorize a second delete, and presence alone cannot prove success.
    fn reconcile_teardown(&mut self, _resource: &Realized, _action: &str) -> Teardown {
        Teardown::Unknown("this provider cannot verify the original teardown".into())
    }
    /// Detach from an adopted resource. It must keep running.
    fn detach(&mut self, resource: &Realized) -> Teardown;
}
