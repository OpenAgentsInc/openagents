# Web fonts

Normal website text uses the original variable Geist v1.401 face, restored
unchanged from commit `6c06f3cc652ef8da7d69fef8db06f3e0a5be7a81`, where it lived
at `apps/coder/src/assets/fonts/geist/geist.ttf`. It supports weights 100–900.
The server serves it from `/fonts/Geist.ttf` on the same origin.

The embedded font license is SIL OFL 1.1. `Geist-OFL.txt` preserves the matching
Geist Project Authors license from commit
`4be3f56c39f5e84481c2fa80a88fbeb0d381e68a`.

Code, tool output, terminal grids, and the OpenAgents wordmark use the existing
[`paper-mono`](../../paper-mono/README.md) face. `static/fonts.css` declares the
web font stacks; each web stylesheet receives those declarations from the server.
