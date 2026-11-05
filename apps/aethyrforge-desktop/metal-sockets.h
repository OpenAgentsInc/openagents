// Platform effects over the shared Coax socket/retained-storage authorities.
#ifndef FORGE_METAL_SOCKETS_H
#define FORGE_METAL_SOCKETS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct {int64_t id,generation,engine;} MNSocketOwner;
typedef struct {MNSocketOwner owner;int64_t event;const uint8_t *bytes;size_t count;void *token;} MNSocketEvent;
uint32_t mn_api_version(void);
bool mn_uri(const uint8_t *bytes,size_t count);
bool mn_opening_sizes(int64_t origin_bytes,int64_t cookie_bytes);
bool mn_opening_context(const uint8_t *origin,size_t origin_count,const uint8_t *cookie,size_t cookie_count);
bool mn_poll_more(int64_t handled,int64_t started_ms,int64_t now_ms);
void *mn_open(int64_t inbox_capacity);
bool mn_start(void *context,MNSocketOwner owner,const uint8_t *url,size_t count);
bool mn_start_context(void *context,MNSocketOwner owner,const uint8_t *url,size_t count,const uint8_t *origin,size_t origin_count,const uint8_t *cookie,size_t cookie_count);
bool mn_send(void *context,MNSocketOwner owner,const uint8_t *bytes,size_t count);
void mn_socket_close(void *context,MNSocketOwner owner);
bool mn_next(void *context,MNSocketEvent *event);
void mn_done(void *token);
// Returns immediately; fixed workers retain old effects until they stop.
void mn_close(void *context);
// Application-owned storage observations, separate from SDK/kernel buffers.
int64_t mn_info(void *context,int32_t field);
int64_t mn_pool_info(int32_t field);
// Process teardown / test-only join. No further contexts may open afterward.
void mn_shutdown(void);
#endif
