//! The tenant-to-artifact registry: who may reach which door, and under
//! which identity.
//!
//! The Decision API's serving half makes two promises that cannot live in
//! code alone: a tenant's key reaches only the doors its agreement binds,
//! and the artifact that answers is the artifact the registry named. This
//! crate is the document both promises are read from — a versioned,
//! self-digested manifest that maps a stable tenant identity to the door
//! names it may use, each door bound to an artifact digest and the
//! execution configuration it was admitted under.
//!
//! # What a binding decides
//!
//! Authorization and identity are the same lookup. [`Registry::authorize`]
//! answers whether a tenant may reach a door and hands back an
//! [`Admission`] — a snapshot of the binding as it stood when the request
//! was admitted. The admission is what the request is served under: a
//! registry update cannot relabel a call already in flight, because the
//! admission carries its own copy of the binding and the registry revision
//! it came from.
//!
//! [`Admission::verify`] then checks the identity a serving process
//! publishes — the model card's artifact signature and execution settings —
//! against the bound expectation. A swapped artifact under a known name is
//! a fault the registry names, and it stays a fault until an explicit
//! update authorizes the replacement.
//!
//! # Revisions, not edits
//!
//! A registry directory holds the current manifest and every manifest that
//! preceded it, archived by digest. [`Registry::update`] is atomic — the
//! new manifest names the digest it supersedes, the file is replaced in
//! one rename, and the old revision remains readable through
//! [`Registry::revision`], so which identity answered an earlier request
//! can always be explained without rewriting anything historical.
//!
//! # What this is not
//!
//! The manifest carries a credential *reference* — a key id or digest —
//! never a secret; the keys themselves are the authentication layer's
//! business. A `trained`-lane binding must name the admission record that
//! authorized the candidate; this crate enforces that the reference exists
//! and leaves deciding which references are honest to the admission
//! contract. And a verified binding still cannot attest remote weights —
//! it proves the serving process claims the identity the registry bound.

pub mod backend;
pub mod keys;
mod manifest;
pub mod quota;
mod registry;

pub use keys::{AuthRefusal, Authenticated, Issued, Key, KeyStore, KeyTrouble, Status};
pub use manifest::{Binding, Capacity, Expected, Lane, Manifest, Quota, SCHEMA, Tenant, lane_name};
pub use registry::{Admission, Fault, Published, Refusal, Registry, Trouble};
