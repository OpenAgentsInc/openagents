// Firm reason: 6 (owner-only 0600 AF_UNIX attach server ~/.openagents/desktop/browser.sock with getpeereid/SO_PEERCRED peer-UID verification).
// Spec: docs/browser/desktop-browser-pane-spec.md (§6.3, §8.1, §9.5); Issue #14108.
#pragma once
#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct OaBrAttachServer OaBrAttachServer;

// Callback invoked on each line-delimited JSON command received from a UID-verified local peer.
// Must write a single-line JSON response (without trailing newline) into resp_buf and return its byte length.
typedef int (*OaBrAttachHandler)(void *ctx, const char *req_json, char *resp_buf, int resp_cap);

// Creates 0700 directory and binds 0600 Unix socket at sock_path (default ~/.openagents/desktop/browser.sock if NULL).
OaBrAttachServer *oa_br_attach_start(const char *sock_path, OaBrAttachHandler handler, void *ctx);

// Non-blocking poll (call from desktop UI/runloop timer). Accepts verified peers and dispatches pending requests.
int oa_br_attach_poll(OaBrAttachServer *srv);

// Returns connected peer count, socket path, and whether the listener is active.
bool oa_br_attach_listening(const OaBrAttachServer *srv);
const char *oa_br_attach_path(const OaBrAttachServer *srv);

// Closes socket and unlinks socket path cleanly.
void oa_br_attach_stop(OaBrAttachServer *srv);

#ifdef __cplusplus
}
#endif
