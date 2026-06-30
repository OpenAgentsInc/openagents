# @openagentsinc/arbiter-effect

Effect schemas and read-only render helpers for Arbiter-style 2D dataflow
graphs.

The initial extraction is intentionally small: `./core` owns the public graph
contract and geometry helpers, while `./foldkit` owns the read-only SVG/HTML
renderer proven first in Khala Code Desktop. Direct manipulation, live
subscriptions, and Three.js/WebGL rendering stay out of this package for now.
