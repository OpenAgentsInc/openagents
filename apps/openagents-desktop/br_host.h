// Firm reason: 6 (engine-neutral C ABI boundary bridging Coax browserpane.coax to native WebViews, CEF OSR helper, and experimental Servo/Ladybird backends).
// Spec: docs/browser/desktop-browser-pane-spec.md (§4, §7.1, §9.4, §11.1); Issues #14107, #14108, #14109, #14110.
#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define OA_BR_MAX_TABS 12
#define OA_BR_DEFAULT_ACTIVE_RENDERERS 4

typedef enum {
    OA_BR_BACKEND_SYSTEM   = 0, // WKWebView (macOS) / WebKitGTK 6 (Linux) / WebView2 (Windows)
    OA_BR_BACKEND_CEF_OSR  = 1, // Signed on-demand oa-browser-helper (CEF Off-Screen Rendering + Native CDP)
    OA_BR_BACKEND_SERVO    = 2, // Experimental Phase 4 plug-in slot (WebRender shared GPU texture)
    OA_BR_BACKEND_LADYBIRD = 3, // Experimental Phase 4 plug-in slot (LibWeb out-of-process)
} OaBrowserBackendKind;

typedef enum {
    OA_BR_PROFILE_WORKSPACE = 0,
    OA_BR_PROFILE_PERSONAL  = 1,
    OA_BR_PROFILE_EPHEMERAL = 2,
} OaBrowserProfile;

typedef enum {
    OA_BR_NAV_BACK    = 0,
    OA_BR_NAV_FORWARD = 1,
    OA_BR_NAV_RELOAD  = 2,
    OA_BR_NAV_STOP    = 3,
} OaBrowserNavAction;

typedef enum {
    OA_BR_LEASE_NONE              = 0,
    OA_BR_LEASE_ACTIVE            = 1,
    OA_BR_LEASE_PAUSED_BY_HUMAN   = 2,
    OA_BR_LEASE_AWAITING_APPROVAL = 3,
} OaBrowserLeaseState;

typedef struct OaBrowserHost OaBrowserHost;

typedef struct {
    void (*on_url)(void *ctx, int tab_id, const char *url);
    void (*on_title)(void *ctx, int tab_id, const char *title);
    void (*on_load_state)(void *ctx, int tab_id, bool loading, bool can_back, bool can_fwd, float progress);
    void (*on_console)(void *ctx, int tab_id, int level, const char *text, const char *source, int line);
    void (*on_network)(void *ctx, int tab_id, const char *method, int status, const char *url, int ms);
    void (*on_dom_pick)(void *ctx, int tab_id, const char *selection_json);
    void (*on_frame_texture)(void *ctx, int tab_id, void *native_handle, int w, int h);
    void (*on_crashed)(void *ctx, int tab_id, const char *reason);
    void (*on_hibernated)(void *ctx, int tab_id, bool hibernated);
    void (*on_lease_changed)(void *ctx, int tab_id, int lease_state, const char *worker_id);
} OaBrowserCallbacks;

// Pluggable backend implementation vtable registered by WKWebView / WebKitGTK / WebView2 / CEF OSR / Servo / Ladybird.
typedef struct {
    OaBrowserBackendKind kind;
    const char *name;
    bool supports_ext_texture;
    bool supports_native_cdp;
    void *(*init)(void *native_parent_view, const OaBrowserCallbacks *cbs, void *cb_ctx);
    void  (*shutdown)(void *impl);
    int   (*tab_open)(void *impl, int tab_id, int profile, const char *initial_url);
    void  (*tab_close)(void *impl, int tab_id);
    void  (*tab_activate)(void *impl, int tab_id);
    void  (*tab_navigate)(void *impl, int tab_id, const char *url);
    void  (*tab_nav_action)(void *impl, int tab_id, int action);
    void  (*tab_set_bounds)(void *impl, int tab_id, float x, float y, float w, float h, float scale, bool visible);
    void  (*tab_set_over_rects)(void *impl, int tab_id, const float *xywh_rects, int count);
    void  (*tab_set_picking)(void *impl, int tab_id, bool enabled);
    void  (*tab_set_muted)(void *impl, int tab_id, bool muted);
    void  (*tab_set_zoom)(void *impl, int tab_id, int zoom_pct);
    void  (*tab_set_hibernated)(void *impl, int tab_id, bool hibernated);
    int   (*tab_exec_cmd)(void *impl, int tab_id, const char *cmd_json, char *resp_buf, int resp_cap);
} OaBrowserBackendOps;

// Register a pluggable backend implementation (replaces any prior registration for the same kind).
bool oa_br_register_backend(const OaBrowserBackendOps *ops);
const OaBrowserBackendOps *oa_br_get_backend(OaBrowserBackendKind kind);

// Host lifecycle
OaBrowserHost *oa_br_create(OaBrowserBackendKind kind, void *native_parent_view, const OaBrowserCallbacks *cbs, void *cb_ctx);
void oa_br_destroy(OaBrowserHost *h);
OaBrowserBackendKind oa_br_backend_kind(const OaBrowserHost *h);
bool oa_br_backend_supports_ext_texture(const OaBrowserHost *h);
bool oa_br_backend_supports_native_cdp(const OaBrowserHost *h);

// Tab lifecycle & LRU hibernation governance (Invariant: active renderers <= max_active_renderers, default 4)
int  oa_br_tab_open(OaBrowserHost *h, int profile, const char *initial_url);
void oa_br_tab_close(OaBrowserHost *h, int tab_id);
void oa_br_tab_activate(OaBrowserHost *h, int tab_id);
void oa_br_tab_navigate(OaBrowserHost *h, int tab_id, const char *url);
void oa_br_tab_nav_action(OaBrowserHost *h, int tab_id, int action);
void oa_br_tab_set_bounds(OaBrowserHost *h, int tab_id, float x, float y, float w, float h_px, float scale, bool visible);
void oa_br_tab_set_over_rects(OaBrowserHost *h, int tab_id, const float *xywh_rects, int count);
void oa_br_tab_set_picking(OaBrowserHost *h, int tab_id, bool enabled);
void oa_br_tab_set_muted(OaBrowserHost *h, int tab_id, bool muted);
void oa_br_tab_set_zoom(OaBrowserHost *h, int tab_id, int zoom_pct);
void oa_br_set_max_active_renderers(OaBrowserHost *h, int cap);
int  oa_br_active_renderer_count(const OaBrowserHost *h);
bool oa_br_tab_is_hibernated(const OaBrowserHost *h, int tab_id);

// Human Supremacy & Agent Lease arbitration (Invariant I-5)
int  oa_br_lease_acquire(OaBrowserHost *h, int tab_id, const char *worker_id);
bool oa_br_lease_human_takeover(OaBrowserHost *h, int tab_id);
bool oa_br_lease_resume(OaBrowserHost *h, int tab_id);
void oa_br_lease_release(OaBrowserHost *h, int tab_id);
int  oa_br_lease_state(const OaBrowserHost *h, int tab_id);

// Canonical 12-verb agent automation dispatch (openagents.desktop.browser-cmd.v1 JSON in/out)
// Enforces Human Supremacy leases, sensitive-action approval gates, and automatic secret redaction.
int oa_br_tab_exec_cmd(OaBrowserHost *h, int tab_id, const char *cmd_json, char *resp_buf, int resp_cap);

// Secret redactor utility (strips Authorization / Cookie / Set-Cookie / Bearer / password fields)
int oa_br_redact_secrets(const char *src, char *out, int out_cap);

// Unified JS bridge payload shared by WKWebView, WebKitGTK 6, and WebView2
const char *oa_br_unified_bridge_js(void);

#ifdef __cplusplus
}
#endif
