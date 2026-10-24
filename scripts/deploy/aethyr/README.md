# Aethyr engine release tools

`release.sh` builds, stages, checks, and pins one engine release from one
openagents commit. `render.py` makes the editor and preview container
contexts, `package-assets.py` freezes a scene's compiled asset closure, and
`cloudbuild.yaml` builds a context into an image.

The procedure, prerequisites, and release history are in
[`docs/verse/aethyrforge-deployment.md`](../../../docs/verse/aethyrforge-deployment.md).
