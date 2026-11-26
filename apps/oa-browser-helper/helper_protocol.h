// Firm reason: 6 (out-of-process IPC + zero-copy GPU texture frame header between OpenAgents Desktop and signed oa-browser-helper).
// Spec: docs/browser/desktop-browser-pane-spec.md (§4, §7.1 Path 2, §11.1 Phase 3); Issue #14109.
#pragma once
#include <stdint.h>

#define OA_BR_HELPER_PROTOCOL_V1 1

typedef enum {
    OA_BR_GPU_HANDLE_NONE      = 0,
    OA_BR_GPU_HANDLE_IOSURFACE = 1, // macOS IOSurfaceID / Mach port -> MTLTexture
    OA_BR_GPU_HANDLE_DMABUF    = 2, // Linux DMA-BUF fd via SCM_RIGHTS -> VK_EXT_external_memory_dma_buf
    OA_BR_GPU_HANDLE_DXGI_NT   = 3, // Windows NT shared HANDLE -> VK_KHR_external_memory_win32
} OaBrGpuHandleKind;

typedef struct {
    uint32_t magic;       // 0x4F414252 ("OABR")
    uint32_t version;     // OA_BR_HELPER_PROTOCOL_V1
    uint32_t tab_id;
    uint32_t width;
    uint32_t height;
    uint32_t handle_kind; // OaBrGpuHandleKind
    uint64_t handle_id;   // IOSurfaceID / fd / NT handle
    uint64_t frame_seq;
} OaBrHelperFrameHeader;
