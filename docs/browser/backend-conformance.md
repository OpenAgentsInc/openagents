# Browser Backend Conformance & Sovereignty Plug-In Guide (`br_host.h`)

**Spec:** `docs/browser/desktop-browser-pane-spec.md` (§3 Option C/E, §4, §11.1 Phase 4)  
**Issues:** #14105 (Master), #14110 (Phase 4)

## 1. Engine-Neutral ABI Invariant (`I-2`)

`apps/openagents-desktop/page/browserpane.coax`, `coax/lib/agent/desktop/browser-route.coax`, and `coax/lib/agent/desktop/browser-policy.coax` never reference any specific web engine. Every backend registers an `OaBrowserBackendOps` vtable into `apps/openagents-desktop/br_host.h`:

| `OaBrowserBackendKind` | Status | Compositing Path | Automation |
|---|---|---|---|
| `OA_BR_BACKEND_SYSTEM` (`0`) | Phase 1 Default | Native child view + `cd_over_rects` even-odd cutout mask (`WKWebView` / `WebKitGTK 6` / `WebView2`) | `oa_br_unified_bridge_js()` |
| `OA_BR_BACKEND_CEF_OSR` (`1`) | Phase 3 Pro (on-demand) | Zero-copy shared GPU texture (`IOSurface` / `DMA-BUF` / DXGI) → `composition-v2` slot 1 | Native CDP |
| `OA_BR_BACKEND_SERVO` (`2`) | Phase 4 Experimental Slot | WebRender shared GPU texture → `composition-v2` slot 1 | Bridge / WebDriver |
| `OA_BR_BACKEND_LADYBIRD` (`3`) | Phase 4 Experimental Slot | Out-of-process shared surface → `composition-v2` slot 1 | Bridge / WebDriver |

## 2. Mandatory Conformance Gate

Any new backend (including `OA_BR_BACKEND_SERVO` or `OA_BR_BACKEND_LADYBIRD`) MUST pass the conformance suite before being enabled by default:

1. **Multi-Tab & 4-Renderer LRU Hibernation:** Open 12 tabs (`OA_BR_MAX_TABS`), assert `oa_br_active_renderer_count(h) <= 4`, verify LRU background tabs transition to `hibernated = true` and wake cleanly on `oa_br_tab_activate`.
2. **3-Profile Storage Isolation:** Cookies/IndexedDB/localStorage set in `OA_BR_PROFILE_WORKSPACE` MUST NOT leak into `OA_BR_PROFILE_PERSONAL` or `OA_BR_PROFILE_EPHEMERAL`; `EPHEMERAL` MUST wipe completely on tab close and NEVER persist across restarts.
3. **Human Supremacy (`I-5`):** Calling `oa_br_lease_human_takeover` immediately transitions an active agent lease to `OA_BR_LEASE_PAUSED_BY_HUMAN`, rejecting mutating commands until `oa_br_lease_resume`.
4. **Automatic Secret Redaction:** `oa_br_tab_exec_cmd` MUST scrub `Authorization`, `Cookie`, `Set-Cookie`, `X-Api-Key`, `Bearer`, and `"password"` fields before returning any response over `~/.openagents/desktop/browser.sock`.
5. **Overlay & Thermal Budgets:** Respect `cd_over_rects` cutouts (or `supports_ext_texture = true` into `composition-v2`), emit `0 Hz` frames while hidden (`◫`), and isolate renderer crashes via `on_crashed` without crashing OpenAgents Desktop.

## 3. Running the Conformance Suite

```bash
# C ABI + LRU hibernation + Human Supremacy + 0600 browser.sock + secret redaction + Servo slot
clang -std=gnu11 -Wall -Wextra -Werror \
  apps/openagents-desktop/tests/br_host_conformance_test.c \
  apps/openagents-desktop/br_host.c \
  apps/openagents-desktop/br_attach.c \
  -o /tmp/br_host_conformance_test && /tmp/br_host_conformance_test

# Coax #authority routing, policy, tool, and pane checks
coax/bin/coax test coax/lib/agent/desktop/browser-route.coax --library
coax/bin/coax test coax/lib/agent/desktop/browser-policy.coax --library
coax/bin/coax test coax/lib/agent/tools/browser.coax --library
coax/bin/coax test apps/openagents-desktop/page/checks/browserpane-checks.coax --library

# Standalone signed Phase 3 CEF OSR helper self-test
apps/oa-browser-helper/build.sh --self-test
```
