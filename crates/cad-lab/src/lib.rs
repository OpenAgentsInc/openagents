//! CAD Lab: build parts and assemblies by talking (docs/cad-lab/README.md).
//!
//! Documents are `part.v1` and `assembly.v1` ([`cad::doc`]); every change
//! is a version with a one-line summary, a digest and its check report
//! (each feature rebuilds, every solid closes, no undeclared overlaps, mass
//! properties), kept privately on this computer ([`lab`]). `cad-lab serve`
//! is the local service the web chat's `cad_*` tools, its inline card and
//! `openagents cad` call.

pub mod lab;
