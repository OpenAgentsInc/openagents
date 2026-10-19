# HTMX browser dependencies

These pinned third-party libraries run HTTP fragment swaps and SSE delivery.
The Rust server supplies the markup, and the Rust browser adapter supplies local
input behavior. Both libraries are served from this origin. Pages disable
evaluated expressions, returned scripts, and persistent history snapshots.

| File | Source | SHA-256 |
| --- | --- | --- |
| `htmx-2.0.11.min.js` | [HTMX 2.0.11 distribution](https://cdn.jsdelivr.net/npm/htmx.org@2.0.11/dist/htmx.min.js) | `d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717` |
| `htmx-sse-2.2.4.js` | [SSE 2.2.4 distribution](https://cdn.jsdelivr.net/npm/htmx-ext-sse@2.2.4/sse.js) | `3b5992a541619babefc4c169505af474df5c3039da51e59b96ccf9241ecd61d2` |

The adjacent license files retain each project's BSD Zero Clause terms. See
[HTMX configuration](https://htmx.org/docs/#configuration) and the
[SSE extension](https://htmx.org/extensions/sse/).

## three.js

`/lander3` draws the Grid with three.js 0.184.0 (the version `three-effect`
pins), served from this origin at `/lander3/three/`. The module imports the
core beside it as `./three.core.min.js`. `three-0.184.0/LICENSE` keeps its
MIT terms.

| File | Source | SHA-256 |
| --- | --- | --- |
| `three-0.184.0/three.module.min.js` | [three 0.184.0 on npm](https://registry.npmjs.org/three/-/three-0.184.0.tgz), `build/` | `36a60b0120335f89a80a0dab70292292b0ec414b3d05e83cd09a3ea428c6712a` |
| `three-0.184.0/three.core.min.js` | [three 0.184.0 on npm](https://registry.npmjs.org/three/-/three-0.184.0.tgz), `build/` | `6486aa0d719cfa87ec88dc47223b59b1fb8417a1a407fc0e52467c943e2f8cc9` |

## three.js

`/lander3` draws the Grid with three.js 0.184.0 (the version `three-effect`
pins), served from this origin at `/lander3/three/`. The module imports the
core beside it as `./three.core.min.js`. `three-0.184.0/LICENSE` keeps its
MIT terms.

| File | Source | SHA-256 |
| --- | --- | --- |
| `three-0.184.0/three.module.min.js` | [three 0.184.0 on npm](https://registry.npmjs.org/three/-/three-0.184.0.tgz), `build/` | `36a60b0120335f89a80a0dab70292292b0ec414b3d05e83cd09a3ea428c6712a` |
| `three-0.184.0/three.core.min.js` | [three 0.184.0 on npm](https://registry.npmjs.org/three/-/three-0.184.0.tgz), `build/` | `6486aa0d719cfa87ec88dc47223b59b1fb8417a1a407fc0e52467c943e2f8cc9` |
