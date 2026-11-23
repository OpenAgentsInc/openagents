// Firm reason: 6 (opaque C ABI carrier over Generated/client_core.h for iOS & macOS Swift; all crypto/protocol/policy rules live in Coax).
#include "client_core.h"
#include "client-core-bridge.h"
#include "../native/tailnet-codec.c"

#include <stdlib.h>
#include <string.h>

typedef struct {
    coax_t_DvWork dv;
    coax_t_EaRequest req;
    coax_t_EaView view;
    coax_t_WsRequest ws_req;
    coax_t_WsView ws_view;
} oa_mobile_work_t;

static coax_string oa_mc_cstr(const char *s) {
    return (coax_string){ s ? s : "", s ? (int64_t)strlen(s) : 0 };
}

static coax_string oa_mc_bytes(const uint8_t *p, int64_t n) {
    return (coax_string){ p ? (const char *)p : "", (p && n > 0) ? n : 0 };
}

static coax_v_u8 oa_mc_in(const uint8_t *p, int64_t n) {
    return (coax_v_u8){ (uint8_t *)p, (p && n > 0) ? n : 0 };
}

static coax_v_u8 oa_mc_out(uint8_t *p, int64_t n) {
    return (coax_v_u8){ p, (p && n > 0) ? n : 0 };
}

void *oa_mobile_work_open(void) {
    return calloc(1, sizeof(oa_mobile_work_t));
}

void oa_mobile_work_close(void *work) {
    if (!work) return;
    memset(work, 0, sizeof(oa_mobile_work_t));
    free(work);
}

int64_t oa_mobile_public(const uint8_t secret[32], uint8_t out[64]) {
    return dv_public(oa_mc_in(secret, 32), oa_mc_out(out, 64));
}

int64_t oa_mobile_seal(
    void *work,
    int64_t op,
    const uint8_t secret[32],
    const char *host,
    const char *relay,
    const char *arg,
    int64_t epoch,
    const uint8_t *params,
    int64_t params_n,
    int64_t now,
    const uint8_t random[96],
    uint8_t *out,
    int64_t cap
) {
    if (!work) return -1;
    oa_mobile_work_t *w = (oa_mobile_work_t *)work;
    return dv_seal(
        (coax_v_t_DvWork){ &w->dv, 1 },
        op,
        oa_mc_in(secret, 32),
        oa_mc_cstr(host),
        oa_mc_cstr(relay),
        oa_mc_cstr(arg),
        epoch,
        oa_mc_bytes(params, params_n),
        now,
        oa_mc_in(random, 96),
        oa_mc_out(out, cap)
    );
}

int64_t oa_mobile_event_id(void *work, uint8_t out[64]) {
    if (!work) return -1;
    oa_mobile_work_t *w = (oa_mobile_work_t *)work;
    return dv_event_id((coax_v_t_DvWork){ &w->dv, 1 }, oa_mc_out(out, 64));
}

int64_t oa_mobile_open(
    void *work,
    const uint8_t secret[32],
    const char *host,
    const uint8_t *event,
    int64_t event_n,
    int64_t now,
    uint8_t *out,
    int64_t cap
) {
    if (!work) return -1;
    oa_mobile_work_t *w = (oa_mobile_work_t *)work;
    return dv_open(
        (coax_v_t_DvWork){ &w->dv, 1 },
        oa_mc_in(secret, 32),
        oa_mc_cstr(host),
        oa_mc_bytes(event, event_n),
        now,
        oa_mc_out(out, cap)
    );
}

int oa_mobile_host_key_ok(const char *key) {
    return mc_host_key_ok(oa_mc_cstr(key)) ? 1 : 0;
}

int oa_mobile_is_agy_model(const char *id) {
    return mc_is_agy_model(oa_mc_cstr(id)) ? 1 : 0;
}

int64_t oa_mobile_normalize_agy_model(const char *want, uint8_t *out, int64_t cap) {
    return mc_normalize_agy_model(oa_mc_cstr(want), oa_mc_out(out, cap));
}

int64_t oa_mobile_normalize_agy_effort(const char *model, const char *want, uint8_t *out, int64_t cap) {
    return mc_normalize_agy_effort(oa_mc_cstr(model), oa_mc_cstr(want), oa_mc_out(out, cap));
}

int oa_mobile_folder_ok(const char *folder) {
    return mc_folder_ok(oa_mc_cstr(folder)) ? 1 : 0;
}

int oa_mobile_mode_ok(const char *mode) {
    return mc_mode_ok(oa_mc_cstr(mode)) ? 1 : 0;
}

int oa_mobile_v1_ea_compat(const uint8_t *raw, int64_t n) {
    return mc_v1_ea_compat(oa_mc_bytes(raw, n)) ? 1 : 0;
}

int64_t oa_mobile_env_request(
    void *work,
    const char *method,
    const char *session,
    const char *command,
    int64_t generation,
    int64_t revision,
    const char *text,
    const char *interaction,
    int allow,
    int64_t before,
    int64_t issued,
    int64_t expires,
    const char *model,
    const char *effort,
    const char *folder,
    const char *item,
    const char *mode,
    const char *option_id,
    const char *attachments,
    uint8_t *out,
    int64_t cap
) {
    if (!work) return -1;
    oa_mobile_work_t *w = (oa_mobile_work_t *)work;
    return mc_env_request(
        (coax_v_t_EaRequest){ &w->req, 1 },
        oa_mc_cstr(method),
        oa_mc_cstr(session),
        oa_mc_cstr(command),
        generation,
        revision,
        oa_mc_cstr(text),
        oa_mc_cstr(interaction),
        allow != 0,
        before,
        issued,
        expires,
        oa_mc_cstr(model),
        oa_mc_cstr(effort),
        oa_mc_cstr(folder),
        oa_mc_cstr(item),
        oa_mc_cstr(mode),
        oa_mc_cstr(option_id),
        oa_mc_cstr(attachments),
        oa_mc_out(out, cap)
    );
}

int oa_mobile_view_valid(void *work, const char *host, const char *method, const uint8_t *raw, int64_t raw_n) {
    if (!work) return 0;
    oa_mobile_work_t *w = (oa_mobile_work_t *)work;
    return mc_view_valid(
        (coax_v_t_EaView){ &w->view, 1 },
        oa_mc_cstr(host),
        oa_mc_cstr(method),
        oa_mc_bytes(raw, raw_n)
    ) ? 1 : 0;
}

int64_t oa_mobile_ws_request(
    void *work,
    const char *method,
    const char *workspace,
    const char *branch,
    const char *base,
    const char *title,
    const char *body,
    int64_t number,
    int64_t page,
    int64_t cursor,
    const char *expected,
    const char *command,
    int64_t issued,
    int64_t expires,
    uint8_t *out,
    int64_t cap
) {
    if (!work) return -1;
    oa_mobile_work_t *w = (oa_mobile_work_t *)work;
    return mc_ws_request(
        (coax_v_t_WsRequest){ &w->ws_req, 1 },
        oa_mc_cstr(method),
        oa_mc_cstr(workspace),
        oa_mc_cstr(branch),
        oa_mc_cstr(base),
        oa_mc_cstr(title),
        oa_mc_cstr(body),
        number,
        page,
        cursor,
        oa_mc_cstr(expected),
        oa_mc_cstr(command),
        issued,
        expires,
        oa_mc_out(out, cap)
    );
}

int oa_mobile_ws_view_valid(void *work, const char *host, const char *method, const uint8_t *raw, int64_t raw_n) {
    if (!work) return 0;
    oa_mobile_work_t *w = (oa_mobile_work_t *)work;
    return mc_ws_view_valid(
        (coax_v_t_WsView){ &w->ws_view, 1 },
        oa_mc_cstr(host),
        oa_mc_cstr(method),
        oa_mc_bytes(raw, raw_n)
    ) ? 1 : 0;
}
