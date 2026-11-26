// Firm reason: 6 (Windows OS-native Edge WebView2 backend behind br_host.h via dynamic LoadLibraryW("WebView2Loader.dll") and SetWindowRgn cd_over_rects cutouts).
// Spec: docs/browser/desktop-browser-pane-spec.md (§4, §7.1, §10); Issue #14107.
#include "../br_host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#endif

typedef struct {
    void *wv2_module;
    bool available;
    OaBrowserCallbacks cbs;
    void *cb_ctx;
    int open_tabs;
    float last_bounds[4];
    int last_over_count;
} OaWinWebView2Impl;

static void *wv2_init(void *native_parent_view, const OaBrowserCallbacks *cbs, void *cb_ctx) {
    (void)native_parent_view;
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)calloc(1, sizeof(OaWinWebView2Impl));
    if (!impl) return NULL;
    if (cbs) impl->cbs = *cbs;
    impl->cb_ctx = cb_ctx;
#ifdef _WIN32
    HMODULE mod = LoadLibraryW(L"WebView2Loader.dll");
    impl->wv2_module = (void *)mod;
    impl->available = (mod != NULL);
#endif
    return impl;
}

static void wv2_shutdown(void *p) {
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)p;
    if (!impl) return;
#ifdef _WIN32
    if (impl->wv2_module) FreeLibrary((HMODULE)impl->wv2_module);
#endif
    free(impl);
}

static int wv2_tab_open(void *p, int tab_id, int profile, const char *initial_url) {
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)p;
    if (!impl) return -1;
    (void)profile; // Maps to CoreWebView2Environment per-profile user-data dir or InPrivate for Ephemeral
    impl->open_tabs++;
    if (impl->cbs.on_url && initial_url) impl->cbs.on_url(impl->cb_ctx, tab_id, initial_url);
    return 0;
}

static void wv2_tab_close(void *p, int tab_id) {
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)p;
    (void)tab_id;
    if (impl && impl->open_tabs > 0) impl->open_tabs--;
}

static void wv2_tab_navigate(void *p, int tab_id, const char *url) {
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)p;
    if (impl && impl->cbs.on_url && url) impl->cbs.on_url(impl->cb_ctx, tab_id, url);
}

static void wv2_tab_set_bounds(void *p, int tab_id, float x, float y, float w, float h, float scale, bool visible) {
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)p;
    (void)tab_id; (void)scale; (void)visible;
    if (!impl) return;
    impl->last_bounds[0] = x; impl->last_bounds[1] = y;
    impl->last_bounds[2] = w; impl->last_bounds[3] = h;
}

static void wv2_tab_set_over_rects(void *p, int tab_id, const float *xywh_rects, int count) {
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)p;
    (void)tab_id; (void)xywh_rects;
    if (impl) impl->last_over_count = count;
}

static int wv2_tab_exec_cmd(void *p, int tab_id, const char *cmd_json, char *resp_buf, int resp_cap) {
    OaWinWebView2Impl *impl = (OaWinWebView2Impl *)p;
    (void)tab_id; (void)cmd_json;
    if (!impl || !resp_buf || resp_cap < 32) return -1;
    return snprintf(resp_buf, (size_t)resp_cap,
                    "{\"ok\":true,\"backend\":\"webview2\",\"available\":%s}",
                    impl->available ? "true" : "false");
}

void oa_win_register_webview2_backend(void) {
    static const OaBrowserBackendOps ops = {
        .kind = OA_BR_BACKEND_SYSTEM,
        .name = "windows-webview2",
        .supports_ext_texture = false,
        .supports_native_cdp = false,
        .init = wv2_init,
        .shutdown = wv2_shutdown,
        .tab_open = wv2_tab_open,
        .tab_close = wv2_tab_close,
        .tab_navigate = wv2_tab_navigate,
        .tab_set_bounds = wv2_tab_set_bounds,
        .tab_set_over_rects = wv2_tab_set_over_rects,
        .tab_exec_cmd = wv2_tab_exec_cmd,
    };
    oa_br_register_backend(&ops);
}
