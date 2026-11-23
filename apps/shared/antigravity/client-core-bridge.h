// Firm reason: 6 (opaque C ABI boundary Swift sees for apps/shared/antigravity/client-core.coax across iOS and macOS; all crypto/protocol/policy rules live in Coax).
#ifndef OA_ANTIGRAVITY_CLIENT_CORE_BRIDGE_H
#define OA_ANTIGRAVITY_CLIENT_CORE_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    OA_ERR_OK = 0,
    OA_ERR_NULL_WORK = -1,
    OA_ERR_TEXT_TOO_LONG = -2,
    OA_ERR_INVALID_MODEL = -3,
    OA_ERR_INVALID_FOLDER = -4,
    OA_ERR_INVALID_MODE = -5,
    OA_ERR_CRYPTO_FAIL = -6,
    OA_ERR_BUFFER_SMALL = -7,
    OA_ERR_JSON_SYNTAX = -8,
    OA_ERR_SCHEMA_MISMATCH = -9,
    OA_ERR_ENCODE_FAIL = -10
} oa_err_t;

void *oa_mobile_work_open(void);
void oa_mobile_work_close(void *work);
int oa_mobile_last_error(void *work);

int64_t oa_mobile_public(const uint8_t secret[32], uint8_t out[64]);
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
    int64_t cap);
int64_t oa_mobile_event_id(void *work, uint8_t out[64]);
int64_t oa_mobile_open(
    void *work,
    const uint8_t secret[32],
    const char *host,
    const uint8_t *event,
    int64_t event_n,
    int64_t now,
    uint8_t *out,
    int64_t cap);

int oa_mobile_host_key_ok(const char *key);
int oa_mobile_is_agy_model(const char *id);
int64_t oa_mobile_normalize_agy_model(const char *want, uint8_t *out, int64_t cap);
int64_t oa_mobile_normalize_agy_effort(const char *model, const char *want, uint8_t *out, int64_t cap);
int oa_mobile_folder_ok(const char *folder);
int oa_mobile_mode_ok(const char *mode);
int oa_mobile_v1_ea_compat(const uint8_t *raw, int64_t n);

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
    int64_t cap);

int oa_mobile_view_valid(void *work, const char *host, const char *method, const uint8_t *raw, int64_t raw_n);

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
    int64_t cap);

int oa_mobile_ws_view_valid(void *work, const char *host, const char *method, const uint8_t *raw, int64_t raw_n);

int64_t oa_mobile_dm_request(
    void *work,
    const char *method,
    const char *session,
    const char *command,
    const char *title,
    const char *folder,
    const char *text,
    int64_t pinned_flag,
    int64_t archived_flag,
    int64_t issued,
    int64_t expires,
    uint8_t *out,
    int64_t cap);

int oa_mobile_dm_view_valid(void *work, const char *host, const char *method, const uint8_t *raw, int64_t raw_n);

#ifdef __cplusplus
}
#endif

#endif
