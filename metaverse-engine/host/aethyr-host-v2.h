// Context-safe multi-instance native host ABI (aethyr.host.v2, GitHub #14042).
// Passes explicit AethyrHostCtx* into native host imports so multiple live 3D views
// (right-side Metaverse pane + inline OA-SHOW-LIVE card + cached tabs + realms)
// coexist safely in one process with zero static global-state crosstalk and a shared
// bounded aggregate VRAM budget.
#ifndef METAVERSE_ENGINE_AETHYR_HOST_V2_H
#define METAVERSE_ENGINE_AETHYR_HOST_V2_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define AETHYR_HOST_ABI_V2 2u
#define AETHYR_HOST_MAX_INSTANCES 16
#define AETHYR_HOST_DEFAULT_BUDGET_BYTES (512ull * 1024ull * 1024ull)
#define AETHYR_HOST_MIN_BUDGET_BYTES     (64ull * 1024ull * 1024ull)
#define AETHYR_HOST_ACTIVE_TAB_BUDGET    (384ull * 1024ull * 1024ull)
#define AETHYR_HOST_INLINE_CARD_BUDGET   (96ull * 1024ull * 1024ull)

typedef enum AethyrInstanceKind {
    AETHYR_INSTANCE_VIEWER = 0,
    AETHYR_INSTANCE_REALM = 1,
    AETHYR_INSTANCE_GAMEBUILDER = 2,
    AETHYR_INSTANCE_INLINE_CARD = 3,
} AethyrInstanceKind;

typedef struct AethyrCameraPose {
    float target[3];
    float distance;
    float azimuth;
    float elevation;
    uint32_t z_up;
    uint32_t fly_mode;
} AethyrCameraPose;

typedef struct AethyrHostCtx AethyrHostCtx;

struct AethyrHostCtx {
    uint32_t abi_version;   // Must equal AETHYR_HOST_ABI_V2
    uint32_t instance_id;   // Unique per-instance token within process
    void *host;             // Opaque platform host instance (AEPageHost*, MFRealm*, RealmHost*)
    void *gpu_budget;       // Optional shared aggregate VRAM budget coordinator
    void *user;             // Optional host-private payload

    // Context-carrying v2 host imports (zero global state required)
    int32_t (*socket_open)(AethyrHostCtx *ctx, uintptr_t url, uint32_t length);
    void    (*socket_send)(AethyrHostCtx *ctx, int32_t id, uintptr_t bytes, uint32_t length);
    void    (*socket_close)(AethyrHostCtx *ctx, int32_t id);
    int32_t (*http_request)(AethyrHostCtx *ctx, uintptr_t url, uint32_t length,
                            uintptr_t body, uint32_t body_length,
                            int32_t flags, int64_t timeout_ms, bool bounded);
    int32_t (*http_cancel)(AethyrHostCtx *ctx, int32_t slot);
    int32_t (*storage_get)(AethyrHostCtx *ctx, int32_t slot, int32_t token,
                           uintptr_t space, int32_t ns, int32_t version,
                           uintptr_t key, int32_t nk);
    int32_t (*storage_put)(AethyrHostCtx *ctx, int32_t slot, int32_t token,
                           uintptr_t space, int32_t ns, int32_t version,
                           uintptr_t key, int32_t nk, uintptr_t bytes, int32_t count);
    int64_t (*now_ms)(AethyrHostCtx *ctx);
    double  (*measure_ms)(AethyrHostCtx *ctx);
};

// Optional producer entrypoint for native multi-instance hosts: when exported by a realm
// or viewer producer, the host calls coaxial_init_v2(state, &hostCtx) before first frame.
typedef void (*AethyrProducerInitV2Fn)(uintptr_t state, AethyrHostCtx *ctx);

// Context-first C import shims exported by compliant native hosts (PageHost.m, vk-realm.c).
int32_t coaxial_v2_socket_open(AethyrHostCtx *ctx, uintptr_t url, uint32_t length);
void    coaxial_v2_socket_send(AethyrHostCtx *ctx, int32_t id, uintptr_t bytes, uint32_t length);
void    coaxial_v2_socket_close(AethyrHostCtx *ctx, int32_t id);
int32_t coaxial_v2_http(AethyrHostCtx *ctx, uintptr_t url, uint32_t length,
                        uintptr_t body, uint32_t body_length, int32_t flags,
                        int64_t timeout_ms, int32_t bounded);
int32_t coaxial_v2_http_cancel(AethyrHostCtx *ctx, int32_t slot);
int32_t coaxial_v2_storage_get(AethyrHostCtx *ctx, int32_t slot, int32_t token,
                               uintptr_t space, int32_t ns, int32_t version,
                               uintptr_t key, int32_t nk);
int32_t coaxial_v2_storage_put(AethyrHostCtx *ctx, int32_t slot, int32_t token,
                               uintptr_t space, int32_t ns, int32_t version,
                               uintptr_t key, int32_t nk, uintptr_t bytes, int32_t count);

#ifdef __cplusplus
}
#endif

#endif // METAVERSE_ENGINE_AETHYR_HOST_V2_H
