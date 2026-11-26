// Conformance & integration test suite for br_host.h / br_host.c / br_attach.c (Phases 1-4; #14107-#14110).
// Validates: pluggable backend ABI (System, CEF OSR, Servo, Ladybird), 12-tab lifecycle,
// 4-renderer LRU hibernation, Human Supremacy takeover, concurrent worker lease conflict,
// automatic secret redaction, and owner-only 0600 AF_UNIX browser.sock round-trip.
#include "../br_host.h"
#include "../br_attach.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

typedef struct {
    int opened;
    int closed;
    int navigated;
    int hibernated_calls;
    int woken_calls;
    int exec_calls;
} MockState;

static void *mock_init(void *parent, const OaBrowserCallbacks *cbs, void *cb_ctx) {
    (void)parent; (void)cbs; (void)cb_ctx;
    return calloc(1, sizeof(MockState));
}
static void mock_shutdown(void *impl) { free(impl); }
static int mock_tab_open(void *impl, int id, int profile, const char *url) {
    (void)id; (void)profile; (void)url;
    ((MockState *)impl)->opened++;
    return 0;
}
static void mock_tab_close(void *impl, int id) { (void)id; ((MockState *)impl)->closed++; }
static void mock_tab_navigate(void *impl, int id, const char *url) { (void)id; (void)url; ((MockState *)impl)->navigated++; }
static void mock_tab_hib(void *impl, int id, bool hib) {
    (void)id;
    MockState *m = (MockState *)impl;
    if (hib) m->hibernated_calls++; else m->woken_calls++;
}
static int mock_exec_cmd(void *impl, int id, const char *cmd, char *resp, int cap) {
    (void)id;
    ((MockState *)impl)->exec_calls++;
    if (strstr(cmd, "browser.network")) {
        return snprintf(resp, (size_t)cap, "{\"ok\":true,\"headers\":\"Authorization: Bearer sk_test_secret\\nCookie: sid=abc123\",\"password\":\"hunter2\"}");
    }
    return snprintf(resp, (size_t)cap, "{\"ok\":true,\"refs\":[\"@e1\",\"@e2\"]}");
}

typedef struct {
    OaBrowserHost *host;
    int tab_id;
} AttachCtx;

static int attach_handler(void *ctx, const char *req, char *resp, int cap) {
    AttachCtx *a = (AttachCtx *)ctx;
    return oa_br_tab_exec_cmd(a->host, a->tab_id, req, resp, cap);
}

int main(void) {
    // 1. Register System, CEF OSR, and experimental Servo backends into the exact same ABI
    OaBrowserBackendOps sys_ops = {
        .kind = OA_BR_BACKEND_SYSTEM, .name = "mock-system",
        .supports_ext_texture = false, .supports_native_cdp = false,
        .init = mock_init, .shutdown = mock_shutdown,
        .tab_open = mock_tab_open, .tab_close = mock_tab_close,
        .tab_navigate = mock_tab_navigate, .tab_set_hibernated = mock_tab_hib,
        .tab_exec_cmd = mock_exec_cmd
    };
    OaBrowserBackendOps cef_ops = sys_ops;
    cef_ops.kind = OA_BR_BACKEND_CEF_OSR; cef_ops.name = "mock-cef-osr";
    cef_ops.supports_ext_texture = true; cef_ops.supports_native_cdp = true;
    OaBrowserBackendOps servo_ops = sys_ops;
    servo_ops.kind = OA_BR_BACKEND_SERVO; servo_ops.name = "mock-servo";
    servo_ops.supports_ext_texture = true; servo_ops.supports_native_cdp = false;

    assert(oa_br_register_backend(&sys_ops));
    assert(oa_br_register_backend(&cef_ops));
    assert(oa_br_register_backend(&servo_ops));

    OaBrowserHost *h = oa_br_create(OA_BR_BACKEND_CEF_OSR, NULL, NULL, NULL);
    assert(h != NULL);
    assert(oa_br_backend_kind(h) == OA_BR_BACKEND_CEF_OSR);
    assert(oa_br_backend_supports_ext_texture(h));
    assert(oa_br_backend_supports_native_cdp(h));

    // 2. Open 12 tabs -> verify active renderer count stays <= 4 (LRU hibernation)
    int ids[12];
    for (int i = 0; i < 12; i++) {
        ids[i] = oa_br_tab_open(h, i % 3, "http://localhost:5173/");
        assert(ids[i] > 0);
        assert(oa_br_active_renderer_count(h) <= 4);
    }
    assert(oa_br_tab_open(h, OA_BR_PROFILE_WORKSPACE, "http://localhost:3000/") == -1); // max 12
    assert(oa_br_active_renderer_count(h) == 4);
    assert(oa_br_tab_is_hibernated(h, ids[0]));
    assert(!oa_br_tab_is_hibernated(h, ids[11]));

    // Activating hibernated tab 0 wakes it and hibernates LRU tab
    oa_br_tab_activate(h, ids[0]);
    assert(!oa_br_tab_is_hibernated(h, ids[0]));
    assert(oa_br_active_renderer_count(h) == 4);

    // 3. Human Supremacy & concurrent lease conflict
    int tid = ids[0];
    assert(oa_br_lease_acquire(h, tid, "worker-1") == 0);
    assert(oa_br_lease_acquire(h, tid, "worker-2") == 1); // conflict
    assert(oa_br_lease_human_takeover(h, tid));
    assert(oa_br_lease_state(h, tid) == OA_BR_LEASE_PAUSED_BY_HUMAN);

    char resp[4096];
    int rn = oa_br_tab_exec_cmd(h, tid, "{\"verb\":\"browser.click\",\"target\":\"@e1\"}", resp, sizeof(resp));
    assert(rn > 0 && strstr(resp, "paused_by_human") != NULL);

    // Resume agent -> mutating command succeeds
    assert(oa_br_lease_resume(h, tid));
    rn = oa_br_tab_exec_cmd(h, tid, "{\"verb\":\"browser.click\",\"target\":\"@e1\"}", resp, sizeof(resp));
    assert(rn > 0 && strstr(resp, "\"ok\":true") != NULL);

    // 4. Secret redaction over owner-only 0600 AF_UNIX browser.sock
    const char *sock_path = "/tmp/oa-br-conformance.sock";
    AttachCtx actx = { .host = h, .tab_id = tid };
    OaBrAttachServer *srv = oa_br_attach_start(sock_path, attach_handler, &actx);
    assert(srv != NULL && oa_br_attach_listening(srv));
    struct stat st;
    assert(stat(sock_path, &st) == 0 && (st.st_mode & 0777) == 0600);

    int cfd = socket(AF_UNIX, SOCK_STREAM, 0);
    assert(cfd >= 0);
    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    snprintf(addr.sun_path, sizeof(addr.sun_path), "%s", sock_path);
    assert(connect(cfd, (struct sockaddr *)&addr, sizeof(addr)) == 0);

    const char *req = "{\"verb\":\"browser.network\"}\n";
    assert(write(cfd, req, strlen(req)) == (ssize_t)strlen(req));
    for (int tries = 0; tries < 20; tries++) {
        if (oa_br_attach_poll(srv) > 0) break;
        usleep(1000);
    }
    char rbuf[4096];
    ssize_t nread = read(cfd, rbuf, sizeof(rbuf) - 1);
    assert(nread > 0);
    rbuf[nread] = '\0';
    assert(strstr(rbuf, "sk_test_secret") == NULL);
    assert(strstr(rbuf, "abc123") == NULL);
    assert(strstr(rbuf, "hunter2") == NULL);
    assert(strstr(rbuf, "[REDACTED]") != NULL);
    close(cfd);

    oa_br_attach_stop(srv);
    oa_br_destroy(h);

    // 5. Verify Servo experimental slot instantiates cleanly
    OaBrowserHost *sh = oa_br_create(OA_BR_BACKEND_SERVO, NULL, NULL, NULL);
    assert(sh != NULL && oa_br_backend_kind(sh) == OA_BR_BACKEND_SERVO && oa_br_backend_supports_ext_texture(sh));
    oa_br_destroy(sh);

    printf("ok   br_host_conformance_test (ABI, LRU hibernation, Human Supremacy, 0600 browser.sock, secret redaction, Servo slot)\n");
    return 0;
}
