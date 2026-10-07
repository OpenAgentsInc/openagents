//! `verse-private`: the owner's command for private Verse assets
//! (`docs/verse/private-assets.md`). Native only; a browser build has no
//! private path.

#[cfg(not(target_arch = "wasm32"))]
mod tool;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    tool::main();
}
