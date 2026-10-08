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
