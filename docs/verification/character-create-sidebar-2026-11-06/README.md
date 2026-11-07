# Character Create compact sidebar

All desktop appearance controls and actions share the left sidebar. Rows use a
40 px pitch and 36 px hit regions; the existing narrow layout stays at the bottom.
The world renders behind the panel, which postprocessing composites at exactly
75% opacity. Three independent pixel samples match the expected blend exactly.
HUD strokes use display colours because the shared host draws them after post.
Scene-only capture excludes the panel. Appearance serialization is unchanged.

Validation: 24 creator checks, 22 glade checks, pointer controls and persistence,
phone layout, camera controls, and save → Everglade → room seat → reload → movement
passed. The handoff requires no duplicate arrival picker. Desktop/phone GPU graphs
have no errors, keep private HDR/post/shadows, and retain the shared battle single
target path. Content-hashed offline shader staging passed. WASM is 615,918 bytes.
The source pack and lighting assets are unchanged; this is a UI change.

The existing preview at port 8817 serves this tested checkout using its existing
API backend at 8847. It retains the same browser origin and saved appearance.
See `live-handoff.log` for the deployed-preview test.
