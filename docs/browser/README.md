# OpenAgents Desktop Browser Specifications

Architecture and implementation specifications for embedding our own first-class browser inside OpenAgents Desktop, following the expandable right-side pane pattern established by the File Viewer (`apps/openagents-desktop/page/filepane.coax`) and the Metaverse 3D Viewer & GameBuilder pane (`apps/openagents-desktop/page/metaversepane.coax`).

## Documents

- **[desktop-browser-pane-spec.md](desktop-browser-pane-spec.md)** — Complete specification covering:
  - How the File Viewer, Metaverse 3D Viewer, and prototype Web Preview (`page/preview.coax`) currently work in `apps/openagents-desktop/`
  - Engine options comparison (OS-Native WebViews, Chromium/CEF bundled & on-demand OSR helper, Servo/Ladybird, Clean-Room Coax engine, and the recommended Pluggable `oa_browser_host` Hybrid Architecture)
  - Responsive right-side pane UX (Split / Expanded `⛶` / Hide `◫` / Narrow), two-row Antigravity chrome, and collapsible DevTools/Agent drawer
  - `#authority` URL routing, security boundaries, and profile/cookie isolation
  - Viewport compositing (`cd_over_rects` hole-cutting vs `composition-v2` zero-copy GPU textures) across macOS, Linux, and Windows
  - Bidirectional Chat ↔ Browser workflows and Human-Supremacy agent automation (CDP / JS bridge + owner-only attach socket)
  - Coax page structs, non-colliding C-ABI IDs, feature flags, `proto/` contracts, and phased delivery plan

## Related Specs

- [../desktop/metaverse-viewer-and-gamebuilder-pane-spec.md](../desktop/metaverse-viewer-and-gamebuilder-pane-spec.md) — Sibling expandable right-side 3D pane spec (`page/metaversepane.coax`)
- [../desktop/openagents-v1-spec.md](../desktop/openagents-v1-spec.md) — `--minimal` release build isolation and feature-flag rules
- [../../components/rendering.md](../../components/rendering.md) — Coax GPU component rendering and external-texture composition
