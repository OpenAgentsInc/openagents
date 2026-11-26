// Firm reason: 6 (engine-neutral C ABI implementation managing tab registry, LRU hibernation, Human Supremacy leases, secret redaction, and pluggable backend dispatch).
// Spec: docs/browser/desktop-browser-pane-spec.md (§4, §7.3, §8, §9.4, §11.1); Issues #14107-#14110.
#include "br_host.h"
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    bool used;
    int id;
    int profile;
    char url[4096];
    char title[256];
    bool hibernated;
    bool muted;
    int zoom_pct;
    uint64_t last_used_tick;
    int lease_state;
    char lease_worker[64];
    bool personal_grant;
} OaBrTabSlot;

struct OaBrowserHost {
    OaBrowserBackendKind kind;
    OaBrowserBackendOps ops;
    bool has_ops;
    void *impl;
    OaBrowserCallbacks cbs;
    void *cb_ctx;
    int max_active_renderers;
    int next_tab_id;
    int active_tab_id;
    uint64_t tick;
    OaBrTabSlot tabs[OA_BR_MAX_TABS];
};

static OaBrowserBackendOps g_backends[4];
static bool g_backend_set[4];

bool oa_br_register_backend(const OaBrowserBackendOps *ops) {
    if (!ops) return false;
    int idx = (int)ops->kind;
    if (idx < 0 || idx >= 4) return false;
    g_backends[idx] = *ops;
    g_backend_set[idx] = true;
    return true;
}

const OaBrowserBackendOps *oa_br_get_backend(OaBrowserBackendKind kind) {
    int idx = (int)kind;
    if (idx < 0 || idx >= 4 || !g_backend_set[idx]) return NULL;
    return &g_backends[idx];
}

static OaBrTabSlot *find_tab(OaBrowserHost *h, int tab_id) {
    if (!h || tab_id <= 0) return NULL;
    for (int i = 0; i < OA_BR_MAX_TABS; i++) {
        if (h->tabs[i].used && h->tabs[i].id == tab_id) return &h->tabs[i];
    }
    return NULL;
}

static const OaBrTabSlot *find_tab_const(const OaBrowserHost *h, int tab_id) {
    if (!h || tab_id <= 0) return NULL;
    for (int i = 0; i < OA_BR_MAX_TABS; i++) {
        if (h->tabs[i].used && h->tabs[i].id == tab_id) return &h->tabs[i];
    }
    return NULL;
}

static void enforce_lru_hibernation(OaBrowserHost *h) {
    if (!h || h->max_active_renderers < 1) return;
    for (;;) {
        int awake = 0;
        for (int i = 0; i < OA_BR_MAX_TABS; i++) {
            if (h->tabs[i].used && !h->tabs[i].hibernated) awake++;
        }
        if (awake <= h->max_active_renderers) break;
        int victim = -1;
        uint64_t oldest = UINT64_MAX;
        for (int i = 0; i < OA_BR_MAX_TABS; i++) {
            if (!h->tabs[i].used || h->tabs[i].hibernated) continue;
            if (h->tabs[i].id == h->active_tab_id) continue;
            if (h->tabs[i].last_used_tick < oldest) {
                oldest = h->tabs[i].last_used_tick;
                victim = i;
            }
        }
        if (victim < 0) break;
        h->tabs[victim].hibernated = true;
        if (h->has_ops && h->ops.tab_set_hibernated) {
            h->ops.tab_set_hibernated(h->impl, h->tabs[victim].id, true);
        }
        if (h->cbs.on_hibernated) {
            h->cbs.on_hibernated(h->cb_ctx, h->tabs[victim].id, true);
        }
    }
}

OaBrowserHost *oa_br_create(OaBrowserBackendKind kind, void *native_parent_view, const OaBrowserCallbacks *cbs, void *cb_ctx) {
    OaBrowserHost *h = (OaBrowserHost *)calloc(1, sizeof(OaBrowserHost));
    if (!h) return NULL;
    h->kind = kind;
    h->max_active_renderers = OA_BR_DEFAULT_ACTIVE_RENDERERS;
    h->next_tab_id = 1;
    h->cb_ctx = cb_ctx;
    if (cbs) h->cbs = *cbs;
    const OaBrowserBackendOps *ops = oa_br_get_backend(kind);
    if (ops) {
        h->ops = *ops;
        h->has_ops = true;
        if (ops->init) h->impl = ops->init(native_parent_view, &h->cbs, cb_ctx);
    }
    return h;
}

void oa_br_destroy(OaBrowserHost *h) {
    if (!h) return;
    if (h->has_ops && h->ops.shutdown && h->impl) {
        h->ops.shutdown(h->impl);
    }
    free(h);
}

OaBrowserBackendKind oa_br_backend_kind(const OaBrowserHost *h) {
    return h ? h->kind : OA_BR_BACKEND_SYSTEM;
}

bool oa_br_backend_supports_ext_texture(const OaBrowserHost *h) {
    return h && h->has_ops && h->ops.supports_ext_texture;
}

bool oa_br_backend_supports_native_cdp(const OaBrowserHost *h) {
    return h && h->has_ops && h->ops.supports_native_cdp;
}

int oa_br_tab_open(OaBrowserHost *h, int profile, const char *initial_url) {
    if (!h) return -1;
    int slot = -1;
    for (int i = 0; i < OA_BR_MAX_TABS; i++) {
        if (!h->tabs[i].used) { slot = i; break; }
    }
    if (slot < 0) return -1;
    OaBrTabSlot *t = &h->tabs[slot];
    memset(t, 0, sizeof(*t));
    t->used = true;
    t->id = h->next_tab_id++;
    t->profile = (profile >= 0 && profile <= 2) ? profile : OA_BR_PROFILE_WORKSPACE;
    t->zoom_pct = 100;
    t->last_used_tick = ++h->tick;
    if (initial_url) snprintf(t->url, sizeof(t->url), "%s", initial_url);
    h->active_tab_id = t->id;
    if (h->has_ops && h->ops.tab_open) {
        h->ops.tab_open(h->impl, t->id, t->profile, initial_url ? initial_url : "");
    }
    enforce_lru_hibernation(h);
    return t->id;
}

void oa_br_tab_close(OaBrowserHost *h, int tab_id) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return;
    if (h->has_ops && h->ops.tab_close) h->ops.tab_close(h->impl, tab_id);
    memset(t, 0, sizeof(*t));
    if (h->active_tab_id == tab_id) {
        h->active_tab_id = 0;
        for (int i = 0; i < OA_BR_MAX_TABS; i++) {
            if (h->tabs[i].used) { h->active_tab_id = h->tabs[i].id; break; }
        }
    }
}

void oa_br_tab_activate(OaBrowserHost *h, int tab_id) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return;
    h->active_tab_id = tab_id;
    t->last_used_tick = ++h->tick;
    if (t->hibernated) {
        t->hibernated = false;
        if (h->has_ops && h->ops.tab_set_hibernated) {
            h->ops.tab_set_hibernated(h->impl, tab_id, false);
        }
        if (h->cbs.on_hibernated) h->cbs.on_hibernated(h->cb_ctx, tab_id, false);
    }
    if (h->has_ops && h->ops.tab_activate) h->ops.tab_activate(h->impl, tab_id);
    enforce_lru_hibernation(h);
}

void oa_br_tab_navigate(OaBrowserHost *h, int tab_id, const char *url) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t || !url) return;
    snprintf(t->url, sizeof(t->url), "%s", url);
    t->last_used_tick = ++h->tick;
    if (h->has_ops && h->ops.tab_navigate) h->ops.tab_navigate(h->impl, tab_id, url);
}

void oa_br_tab_nav_action(OaBrowserHost *h, int tab_id, int action) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return;
    if (h->has_ops && h->ops.tab_nav_action) h->ops.tab_nav_action(h->impl, tab_id, action);
}

void oa_br_tab_set_bounds(OaBrowserHost *h, int tab_id, float x, float y, float w, float h_px, float scale, bool visible) {
    if (!h) return;
    if (h->has_ops && h->ops.tab_set_bounds) h->ops.tab_set_bounds(h->impl, tab_id, x, y, w, h_px, scale, visible);
}

void oa_br_tab_set_over_rects(OaBrowserHost *h, int tab_id, const float *xywh_rects, int count) {
    if (!h) return;
    if (h->has_ops && h->ops.tab_set_over_rects) h->ops.tab_set_over_rects(h->impl, tab_id, xywh_rects, count);
}

void oa_br_tab_set_picking(OaBrowserHost *h, int tab_id, bool enabled) {
    if (!h) return;
    if (h->has_ops && h->ops.tab_set_picking) h->ops.tab_set_picking(h->impl, tab_id, enabled);
}

void oa_br_tab_set_muted(OaBrowserHost *h, int tab_id, bool muted) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return;
    t->muted = muted;
    if (h->has_ops && h->ops.tab_set_muted) h->ops.tab_set_muted(h->impl, tab_id, muted);
}

void oa_br_tab_set_zoom(OaBrowserHost *h, int tab_id, int zoom_pct) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return;
    if (zoom_pct < 25) zoom_pct = 25;
    if (zoom_pct > 500) zoom_pct = 500;
    t->zoom_pct = zoom_pct;
    if (h->has_ops && h->ops.tab_set_zoom) h->ops.tab_set_zoom(h->impl, tab_id, zoom_pct);
}

void oa_br_set_max_active_renderers(OaBrowserHost *h, int cap) {
    if (!h) return;
    if (cap < 1) cap = 1;
    if (cap > OA_BR_MAX_TABS) cap = OA_BR_MAX_TABS;
    h->max_active_renderers = cap;
    enforce_lru_hibernation(h);
}

int oa_br_active_renderer_count(const OaBrowserHost *h) {
    if (!h) return 0;
    int awake = 0;
    for (int i = 0; i < OA_BR_MAX_TABS; i++) {
        if (h->tabs[i].used && !h->tabs[i].hibernated) awake++;
    }
    return awake;
}

bool oa_br_tab_is_hibernated(const OaBrowserHost *h, int tab_id) {
    const OaBrTabSlot *t = find_tab_const(h, tab_id);
    return t ? t->hibernated : false;
}

// ---- Human Supremacy & Lease Arbitration ----

int oa_br_lease_acquire(OaBrowserHost *h, int tab_id, const char *worker_id) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t || !worker_id || !worker_id[0]) return 4;
    if (t->lease_state == OA_BR_LEASE_PAUSED_BY_HUMAN) return 2;
    if (t->lease_state == OA_BR_LEASE_AWAITING_APPROVAL) return 3;
    if (t->lease_state == OA_BR_LEASE_ACTIVE) {
        if (strcmp(t->lease_worker, worker_id) != 0) return 1; // conflict
        return 0;
    }
    t->lease_state = OA_BR_LEASE_ACTIVE;
    snprintf(t->lease_worker, sizeof(t->lease_worker), "%s", worker_id);
    if (h->cbs.on_lease_changed) h->cbs.on_lease_changed(h->cb_ctx, tab_id, t->lease_state, t->lease_worker);
    return 0;
}

bool oa_br_lease_human_takeover(OaBrowserHost *h, int tab_id) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return false;
    if (t->lease_state == OA_BR_LEASE_ACTIVE || t->lease_state == OA_BR_LEASE_AWAITING_APPROVAL) {
        t->lease_state = OA_BR_LEASE_PAUSED_BY_HUMAN;
        if (h->cbs.on_lease_changed) h->cbs.on_lease_changed(h->cb_ctx, tab_id, t->lease_state, t->lease_worker);
        return true;
    }
    return false;
}

bool oa_br_lease_resume(OaBrowserHost *h, int tab_id) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return false;
    if (t->lease_state == OA_BR_LEASE_PAUSED_BY_HUMAN || t->lease_state == OA_BR_LEASE_AWAITING_APPROVAL) {
        t->lease_state = OA_BR_LEASE_ACTIVE;
        if (h->cbs.on_lease_changed) h->cbs.on_lease_changed(h->cb_ctx, tab_id, t->lease_state, t->lease_worker);
        return true;
    }
    return false;
}

void oa_br_lease_release(OaBrowserHost *h, int tab_id) {
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) return;
    t->lease_state = OA_BR_LEASE_NONE;
    t->lease_worker[0] = '\0';
    t->personal_grant = false;
    if (h->cbs.on_lease_changed) h->cbs.on_lease_changed(h->cb_ctx, tab_id, t->lease_state, "");
}

int oa_br_lease_state(const OaBrowserHost *h, int tab_id) {
    const OaBrTabSlot *t = find_tab_const(h, tab_id);
    return t ? t->lease_state : OA_BR_LEASE_NONE;
}

// ---- Secret Redaction ----

static bool ci_prefix(const char *s, const char *pfx) {
    while (*pfx) {
        if (!*s || tolower((unsigned char)*s) != tolower((unsigned char)*pfx)) return false;
        s++; pfx++;
    }
    return true;
}

int oa_br_redact_secrets(const char *src, char *out, int out_cap) {
    if (!src || !out || out_cap <= 0) return 0;
    int n = 0;
    const char *p = src;
    while (*p && n < out_cap - 1) {
        int key_len = 0;
        bool is_json = false;
        if (ci_prefix(p, "authorization:")) key_len = 14;
        else if (ci_prefix(p, "set-cookie:")) key_len = 11;
        else if (ci_prefix(p, "cookie:")) key_len = 7;
        else if (ci_prefix(p, "x-api-key:")) key_len = 10;
        else if (ci_prefix(p, "\"authorization\":\"")) { key_len = 17; is_json = true; }
        else if (ci_prefix(p, "\"cookie\":\"")) { key_len = 10; is_json = true; }
        else if (ci_prefix(p, "\"set-cookie\":\"")) { key_len = 14; is_json = true; }
        else if (ci_prefix(p, "\"password\":\"")) { key_len = 12; is_json = true; }

        if (key_len > 0) {
            for (int k = 0; k < key_len && n < out_cap - 1; k++) out[n++] = p[k];
            p += key_len;
            if (!is_json && *p == ' ' && n < out_cap - 1) out[n++] = *p++;
            const char *tag = "[REDACTED]";
            for (int k = 0; tag[k] && n < out_cap - 1; k++) out[n++] = tag[k];
            if (is_json) {
                while (*p && *p != '"') {
                    if (*p == '\\' && p[1]) { p += 2; continue; }
                    p++;
                }
            } else {
                while (*p && *p != '\n' && *p != '\r') p++;
            }
            continue;
        }
        if (ci_prefix(p, "bearer ")) {
            for (int k = 0; k < 7 && n < out_cap - 1; k++) out[n++] = p[k];
            p += 7;
            const char *tag = "[REDACTED]";
            for (int k = 0; tag[k] && n < out_cap - 1; k++) out[n++] = tag[k];
            while (*p && *p != ' ' && *p != '\n' && *p != '\r' && *p != '"' && *p != ',' && *p != ';') p++;
            continue;
        }
        out[n++] = *p++;
    }
    out[n] = '\0';
    return n;
}

static bool json_has_mutating_verb(const char *cmd) {
    return strstr(cmd, "\"browser.click\"") || strstr(cmd, "\"browser.type\"") ||
           strstr(cmd, "\"browser.press\"") || strstr(cmd, "\"browser.eval\"") ||
           strstr(cmd, "\"browser.navigate\"") || strstr(cmd, "\"browser.open\"");
}

int oa_br_tab_exec_cmd(OaBrowserHost *h, int tab_id, const char *cmd_json, char *resp_buf, int resp_cap) {
    if (!h || !cmd_json || !resp_buf || resp_cap < 32) return -1;
    OaBrTabSlot *t = find_tab(h, tab_id);
    if (!t) {
        return snprintf(resp_buf, (size_t)resp_cap, "{\"ok\":false,\"error\":\"tab_not_found\"}");
    }
    // Enforce Human Supremacy pause before touching backend
    if (t->lease_state == OA_BR_LEASE_PAUSED_BY_HUMAN && json_has_mutating_verb(cmd_json)) {
        return snprintf(resp_buf, (size_t)resp_cap, "{\"ok\":false,\"error\":\"paused_by_human\"}");
    }
    // Block eval on Personal tabs without explicit grant
    if (strstr(cmd_json, "\"browser.eval\"") && t->profile == OA_BR_PROFILE_PERSONAL && !t->personal_grant) {
        return snprintf(resp_buf, (size_t)resp_cap, "{\"ok\":false,\"error\":\"eval_forbidden_on_personal\"}");
    }
    // Wake hibernated tab on command
    if (t->hibernated) oa_br_tab_activate(h, tab_id);

    if (!h->has_ops || !h->ops.tab_exec_cmd) {
        return snprintf(resp_buf, (size_t)resp_cap, "{\"ok\":false,\"error\":\"backend_not_registered\"}");
    }
    char *raw = (char *)malloc((size_t)resp_cap);
    if (!raw) return -1;
    int rn = h->ops.tab_exec_cmd(h->impl, tab_id, cmd_json, raw, resp_cap);
    if (rn < 0) {
        free(raw);
        return snprintf(resp_buf, (size_t)resp_cap, "{\"ok\":false,\"error\":\"backend_exec_failed\"}");
    }
    if (rn >= resp_cap) rn = resp_cap - 1;
    raw[rn] = '\0';
    // Automatically redact any secrets before returning payload to model/agent/MCP
    int out_n = oa_br_redact_secrets(raw, resp_buf, resp_cap);
    free(raw);
    return out_n;
}

// Unified JS bridge injected into WKWebView, WebKitGTK 6, and WebView2.
// Implements DOM picker (openagents.desktop.browser-selection.v1), numbered @e* A11y/DOM snapshot,
// console & fetch/XHR forwarding (scrubbing auth/cookie headers), click ripple, and Vimium-style link hints.
static const char kOaUnifiedBridgeJS[] =
"(function(){\n"
"  if (window.__oaBrowserBridge) return;\n"
"  function post(channel, payload) {\n"
"    try {\n"
"      if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers[channel]) {\n"
"        window.webkit.messageHandlers[channel].postMessage(payload);\n"
"      } else if (window.chrome && window.chrome.webview) {\n"
"        window.chrome.webview.postMessage({channel: channel, payload: payload});\n"
"      }\n"
"    } catch (_) {}\n"
"  }\n"
"  function cssPath(el) {\n"
"    if (!el || el.nodeType !== 1) return '';\n"
"    if (el.id) return '#' + el.id;\n"
"    var parts = [], cur = el;\n"
"    while (cur && cur.nodeType === 1 && parts.length < 5) {\n"
"      var seg = cur.tagName.toLowerCase();\n"
"      if (cur.id) { parts.unshift('#' + cur.id); break; }\n"
"      if (cur.className && typeof cur.className === 'string') {\n"
"        var cls = cur.className.trim().split(/\\s+/).filter(Boolean).slice(0, 2);\n"
"        if (cls.length) seg += '.' + cls.join('.');\n"
"      }\n"
"      parts.unshift(seg); cur = cur.parentElement;\n"
"    }\n"
"    return parts.join(' > ');\n"
"  }\n"
"  function xpathOf(el) {\n"
"    if (!el || el.nodeType !== 1) return '';\n"
"    var segs = [];\n"
"    for (; el && el.nodeType === 1; el = el.parentNode) {\n"
"      var idx = 1;\n"
"      for (var sib = el.previousSibling; sib; sib = sib.previousSibling) {\n"
"        if (sib.nodeType === 1 && sib.tagName === el.tagName) idx++;\n"
"      }\n"
"      segs.unshift(el.tagName.toLowerCase() + '[' + idx + ']');\n"
"    }\n"
"    return '//' + segs.join('/');\n"
"  }\n"
"  // Console capture\n"
"  ['log','warn','error'].forEach(function(lvl, idx) {\n"
"    var orig = console[lvl];\n"
"    console[lvl] = function() {\n"
"      try {\n"
"        var msg = Array.prototype.slice.call(arguments).map(function(a){ return typeof a === 'string' ? a : JSON.stringify(a); }).join(' ').slice(0, 512);\n"
"        post('oaBrConsole', {level: idx, text: msg, source: location.href, line: 0});\n"
"      } catch (_) {}\n"
"      return orig && orig.apply(console, arguments);\n"
"    };\n"
"  });\n"
"  window.addEventListener('error', function(e) {\n"
"    post('oaBrConsole', {level: 2, text: String(e.message || 'Error').slice(0, 512), source: e.filename || location.href, line: e.lineno || 0});\n"
"  });\n"
"  // Fetch instrumentation (no Authorization/Cookie values ever sent)\n"
"  if (window.fetch) {\n"
"    var origFetch = window.fetch;\n"
"    window.fetch = function(input, init) {\n"
"      var t0 = Date.now();\n"
"      var method = (init && init.method) ? String(init.method).toUpperCase() : 'GET';\n"
"      var url = (typeof input === 'string') ? input : (input && input.url ? input.url : '');\n"
"      return origFetch.apply(this, arguments).then(function(res) {\n"
"        post('oaBrNetwork', {method: method, status: res.status || 0, url: String(url).slice(0, 512), ms: Date.now() - t0});\n"
"        return res;\n"
"      }, function(err) {\n"
"        post('oaBrNetwork', {method: method, status: 0, url: String(url).slice(0, 512), ms: Date.now() - t0});\n"
"        throw err;\n"
"      });\n"
"    };\n"
"  }\n"
"  var refMap = {};\n"
"  function snapshot() {\n"
"    refMap = {};\n"
"    var nodes = document.querySelectorAll('a,button,input,textarea,select,[role=button],[role=link],[role=textbox],[tabindex]');\n"
"    var out = [], k = 0;\n"
"    for (var i = 0; i < nodes.length && k < 120; i++) {\n"
"      var el = nodes[i], r = el.getBoundingClientRect();\n"
"      if (r.width < 1 || r.height < 1) continue;\n"
"      var ref = '@e' + (++k);\n"
"      refMap[ref] = el;\n"
"      var isPw = (el.tagName === 'INPUT' && (el.type || '').toLowerCase() === 'password');\n"
"      var val = isPw ? '[REDACTED]' : (el.value ? String(el.value).slice(0, 80) : '');\n"
"      var name = (el.getAttribute('aria-label') || el.innerText || el.placeholder || el.name || '').trim().slice(0, 80);\n"
"      out.push({ref: ref, tag: el.tagName.toLowerCase(), role: el.getAttribute('role') || el.tagName.toLowerCase(), name: name, value: val, x: Math.round(r.left), y: Math.round(r.top), w: Math.round(r.width), h: Math.round(r.height)});\n"
"    }\n"
"    return {url: location.href, title: document.title || '', viewport: [window.innerWidth, window.innerHeight], refs: out};\n"
"  }\n"
"  function showRipple(x, y) {\n"
"    var d = document.createElement('div');\n"
"    d.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;width:24px;height:24px;margin:-12px 0 0 -12px;border-radius:50%;border:2px solid #38bdf8;background:rgba(56,189,248,0.25);left:'+x+'px;top:'+y+'px;';\n"
"    document.documentElement.appendChild(d);\n"
"    setTimeout(function(){ if (d.parentNode) d.parentNode.removeChild(d); }, 320);\n"
"  }\n"
"  function toggleHints(on) {\n"
"    var old = document.getElementById('__oa_hint_layer');\n"
"    if (old && old.parentNode) old.parentNode.removeChild(old);\n"
"    if (!on) return 0;\n"
"    var snap = snapshot();\n"
"    var layer = document.createElement('div');\n"
"    layer.id = '__oa_hint_layer';\n"
"    layer.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:2147483646;';\n"
"    snap.refs.forEach(function(item) {\n"
"      var tag = document.createElement('span');\n"
"      tag.textContent = item.ref;\n"
"      tag.style.cssText = 'position:fixed;left:'+item.x+'px;top:'+item.y+'px;background:#facc15;color:#111;font:700 10px monospace;padding:1px 3px;border-radius:2px;';\n"
"      layer.appendChild(tag);\n"
"    });\n"
"    document.documentElement.appendChild(layer);\n"
"    return snap.refs.length;\n"
"  }\n"
"  var pickBox = null;\n"
"  function setPicking(on) {\n"
"    if (pickBox && pickBox.parentNode) { pickBox.parentNode.removeChild(pickBox); pickBox = null; }\n"
"    document.removeEventListener('mousemove', onPickMove, true);\n"
"    document.removeEventListener('click', onPickClick, true);\n"
"    if (!on) return;\n"
"    pickBox = document.createElement('div');\n"
"    pickBox.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;border:2px solid #38bdf8;background:rgba(56,189,248,0.14);border-radius:3px;display:none;';\n"
"    document.documentElement.appendChild(pickBox);\n"
"    document.addEventListener('mousemove', onPickMove, true);\n"
"    document.addEventListener('click', onPickClick, true);\n"
"  }\n"
"  function onPickMove(e) {\n"
"    var el = document.elementFromPoint(e.clientX, e.clientY);\n"
"    if (!el || el === pickBox) return;\n"
"    var r = el.getBoundingClientRect();\n"
"    pickBox.style.display = 'block'; pickBox.style.left = r.left+'px'; pickBox.style.top = r.top+'px'; pickBox.style.width = r.width+'px'; pickBox.style.height = r.height+'px';\n"
"  }\n"
"  function onPickClick(e) {\n"
"    e.preventDefault(); e.stopPropagation();\n"
"    var el = document.elementFromPoint(e.clientX, e.clientY);\n"
"    setPicking(false);\n"
"    if (!el) return;\n"
"    var r = el.getBoundingClientRect(), cs = window.getComputedStyle(el);\n"
"    post('oaBrDomPick', {\n"
"      url: location.href, selector: cssPath(el), xpath: xpathOf(el),\n"
"      tag: (el.tagName || '').toLowerCase(), role: el.getAttribute('role') || (el.tagName || '').toLowerCase(),\n"
"      accessible_name: (el.getAttribute('aria-label') || el.innerText || '').trim().slice(0, 120),\n"
"      text_excerpt: (el.innerText || el.textContent || '').trim().slice(0, 300),\n"
"      bbox_px: [Math.round(r.left), Math.round(r.top), Math.round(r.width), Math.round(r.height)],\n"
"      viewport: [window.innerWidth, window.innerHeight],\n"
"      computed_style: 'display:' + cs.display + ';font-size:' + cs.fontSize,\n"
"      screenshot_path: ''\n"
"    });\n"
"  }\n"
"  window.__oaBrowserBridge = {\n"
"    snapshot: snapshot,\n"
"    setPicking: setPicking,\n"
"    toggleHints: toggleHints,\n"
"    clickRef: function(target) {\n"
"      var el = refMap[target] || document.querySelector(target);\n"
"      if (!el) return {ok:false, error:'not_found'};\n"
"      var r = el.getBoundingClientRect();\n"
"      showRipple(r.left + r.width/2, r.top + r.height/2);\n"
"      el.click();\n"
"      return {ok:true, tag: el.tagName.toLowerCase()};\n"
"    },\n"
"    typeRef: function(target, text) {\n"
"      var el = refMap[target] || document.querySelector(target);\n"
"      if (!el) return {ok:false, error:'not_found'};\n"
"      el.focus();\n"
"      el.value = text;\n"
"      el.dispatchEvent(new Event('input', {bubbles:true}));\n"
"      el.dispatchEvent(new Event('change', {bubbles:true}));\n"
"      return {ok:true};\n"
"    }\n"
"  };\n"
"})();\n";

const char *oa_br_unified_bridge_js(void) {
    return kOaUnifiedBridgeJS;
}
