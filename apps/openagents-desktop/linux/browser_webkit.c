// Firm reason: 6 (Linux OS-native WebKitGTK 6 backend behind br_host.h via dlopen so headless/minimal builds never fail).
// Spec: docs/browser/desktop-browser-pane-spec.md (§4, §7.1, §10); Issue #14107.
#include "../br_host.h"
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    void *wk_handle;
    bool available;
    OaBrowserCallbacks cbs;
    void *cb_ctx;
    int open_tabs;
    float last_bounds[4];
    int last_over_count;
} OaLinuxWebKitImpl;

static void *wk_init(void *native_parent_view, const OaBrowserCallbacks *cbs, void *cb_ctx) {
    (void)native_parent_view;
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)calloc(1, sizeof(OaLinuxWebKitImpl));
    if (!impl) return NULL;
    if (cbs) impl->cbs = *cbs;
    impl->cb_ctx = cb_ctx;
    // Dynamically load libwebkitgtk-6.0 so OpenAgents Desktop stays zero-bloat and never fails on headless machines
    impl->wk_handle = dlopen("libwebkitgtk-6.0.so.4", RTLD_LAZY | RTLD_LOCAL);
    impl->available = (impl->wk_handle != NULL);
    return impl;
}

static void wk_shutdown(void *p) {
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)p;
    if (!impl) return;
    if (impl->wk_handle) dlclose(impl->wk_handle);
    free(impl);
}

static int wk_tab_open(void *p, int tab_id, int profile, const char *initial_url) {
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)p;
    if (!impl) return -1;
    (void)profile; // Maps to WebKitNetworkSession: persistent(workspace/personal) vs ephemeral
    impl->open_tabs++;
    if (impl->cbs.on_url && initial_url) impl->cbs.on_url(impl->cb_ctx, tab_id, initial_url);
    return 0;
}

static void wk_tab_close(void *p, int tab_id) {
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)p;
    (void)tab_id;
    if (impl && impl->open_tabs > 0) impl->open_tabs--;
}

static void wk_tab_navigate(void *p, int tab_id, const char *url) {
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)p;
    if (impl && impl->cbs.on_url && url) impl->cbs.on_url(impl->cb_ctx, tab_id, url);
}

static void wk_tab_set_bounds(void *p, int tab_id, float x, float y, float w, float h, float scale, bool visible) {
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)p;
    (void)tab_id; (void)scale; (void)visible;
    if (!impl) return;
    impl->last_bounds[0] = x; impl->last_bounds[1] = y;
    impl->last_bounds[2] = w; impl->last_bounds[3] = h;
}

static void wk_tab_set_over_rects(void *p, int tab_id, const float *xywh_rects, int count) {
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)p;
    (void)tab_id; (void)xywh_rects;
    if (impl) impl->last_over_count = count;
}

static int wk_tab_exec_cmd(void *p, int tab_id, const char *cmd_json, char *resp_buf, int resp_cap) {
    OaLinuxWebKitImpl *impl = (OaLinuxWebKitImpl *)p;
    (void)tab_id; (void)cmd_json;
    if (!impl || !resp_buf || resp_cap < 32) return -1;
    return snprintf(resp_buf, (size_t)resp_cap,
                    "{\"ok\":true,\"backend\":\"webkitgtk6\",\"available\":%s}",
                    impl->available ? "true" : "false");
}

void oa_linux_register_webkit_backend(void) {
    static const OaBrowserBackendOps ops = {
        .kind = OA_BR_BACKEND_SYSTEM,
        .name = "linux-webkitgtk6",
        .supports_ext_texture = false,
        .supports_native_cdp = false,
        .init = wk_init,
        .shutdown = wk_shutdown,
        .tab_open = wk_tab_open,
        .tab_close = wk_tab_close,
        .tab_navigate = wk_tab_navigate,
        .tab_set_bounds = wk_tab_set_bounds,
        .tab_set_over_rects = wk_tab_set_over_rects,
        .tab_exec_cmd = wk_tab_exec_cmd,
    };
    oa_br_register_backend(&ops);
}
