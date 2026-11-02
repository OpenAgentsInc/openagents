# Copying a built renderer package

The build and publication adapters run in Coax. They keep the original
manifest and product bytes; copying does not create a new shader package.

```sh
coax/web/tool.sh render-stage APP /build/render/PROFILE/render.json STAGE
coax/web/tool.sh verse/native/render-package.coax \
  --package DIR/render.json --app APP --profile PROFILE --output RenderIdentity.h
coax/web/tool.sh verse/native/render-assets.coax \
  --table assets.json --package DIR/render.json --web-root APP --output ASSETS
```

`render-stage` authenticates every WGSL, SPIRV and checked-tree sibling before
writing either destination. It copies the complete package under the manifest's
SHA-256 in `STAGE/assets/build/render/` and `STAGE/site/build/render/`, and prints
the immutable manifest URL. `render-package` authenticates those same inputs,
checks the requested identity and atomically replaces the generated C header.

`render-assets` consumes the existing typed asset table. It checks the package
identity, retains each needed physical file once and preserves the table's
separate delivery rows. Canonical filesystem paths must remain under the
canonical web root. Lazy observations do not cause file reads. Every input is
read before the first destination write; refusal preserves the previous asset
table. Individual files and the table publish through a temporary sibling and
rename. Existing unrelated texture observations stay in the output directory.

The tools have explicit bounds: 256 KiB manifests/tables, 31 shader products
of at most 2 MiB each, 48 asset rows, 128 distinct asset URLs and 256 MiB of
retained asset bytes. These bounds do not change the engine's budgets. Runtime
package compatibility, GPU resource ownership, media acquisition and rendering
qualification are separate work.

The pure copy policies are `staging-policy.coax` and
`asset-staging-policy.coax`. Their checks precede implementation and their
focused mutation runs have no survivors. The old Python implementations were
retained as compressed reference inputs, compared on identical inputs and then
deleted with the caller switches. Exact byte parity includes an emitted Grid
package and the current Everglade asset table and full container. See
`docs/verification/data/aethyr-ch11-2026-11-02/tool-staging-port/qualification.json`
and its asset qualification record.

For a parity replay, extract the reference archive into a private scratch
directory. `check-staging-tools.coax` accepts a reference root containing the
old `coax/web/render_stage.py` and `verse/native/render_package.py` paths.
`check-asset-staging.coax` accepts a root containing the old
`verse/native/render_assets.py` path; its archived reference member can be
copied into that scratch layout. These are archived comparison inputs, not
the build's tools. The normal build calls only the Coax replacements.
