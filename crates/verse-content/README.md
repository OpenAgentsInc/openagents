# Verse content

This crate owns portable compiled-content identity, outfit/equipment admission,
and original furnishing collision admission. The dedicated host and Verse
clients use the same checks. Default features include no renderer, font library,
private reader, or agent.

The `compiler` feature adds Rust procedural assets, spell geometry, licensed
character/prop import, SVG rasterization, inventory provenance, and original
world recipes. The `verse-content` command compiles `ritual` or `observatory`
into a new directory. See the [dedicated host instructions](../verse-host/README.md).

The compiler retains the existing original source-to-world coordinate basis.
Collision admission for furnishings retains the original model whitelist;
authored social geometry uses the validated `verse-world` profile contract.
This boundary does not define arbitrary scripts or a general world editor.
