// Firm reason: 1 (reuse the shared browser ABI, with a larger inbox and request view).
#define DOM_INBOX_CAP 1048576
#include "../../../coax/native/starter/face.c"
EXPORT("app_body") uint32_t app_body(uint32_t s) { return (uintptr_t)raw(s)->outbox.e; }
EXPORT("app_body_len") int32_t app_body_len(uint32_t s) { return raw(s)->out_n; }
