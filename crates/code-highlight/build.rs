//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! `crates/codegen/xai-grok-markdown/build.rs`.
//!
//! With the `grok` feature, dumps two-face's syntaxes plus the patched
//! Swift grammar into one syntect `SyntaxSet` at build time, so the
//! terminal never builds the set at runtime.

fn main() {
    #[cfg(feature = "grok")]
    grok::dump();
}

#[cfg(feature = "grok")]
mod grok {
    use std::env;
    use std::path::Path;

    use syntect::dumps::dump_to_uncompressed_file;
    use syntect::parsing::SyntaxDefinition;

    pub fn dump() {
        println!("cargo:rerun-if-changed=assets/grok-build/Swift.sublime-syntax");
        // Runtime `SyntaxSet::build` of two-face stalls first Read paint (GB-5513 PTY fold).
        let swift = SyntaxDefinition::load_from_str(
            include_str!("assets/grok-build/Swift.sublime-syntax"),
            true,
            None,
        )
        .expect("parse Swift.sublime-syntax");
        let mut builder = two_face::syntax::extra_newlines().into_builder();
        builder.add(swift);
        let set = builder.build();
        dump_to_uncompressed_file(
            &set,
            Path::new(&env::var("OUT_DIR").unwrap()).join("syntaxes.bin"),
        )
        .expect("dump syntaxes");
    }
}
