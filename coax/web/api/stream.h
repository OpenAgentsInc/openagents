// Coax Web Server non-blocking streaming plane and shared SSE helpers (#14088).
// Framing, cursor validation, keepalive cadence, lifetime, backpressure, and
// NOTIFY payload parsing are decided by stream-policy.coax (#authority).
#pragma once
#include "api.h"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/types.h>

// Shared blocking/streaming HTTP & SSE response helpers (used by inference.c and routes).
bool api_stream_write_all(int fd, const void *data, size_t n);
bool api_stream_begin(int fd, int status, const char *content_type, const char *extra_headers);
bool api_stream_begin_sse(int fd, int status, const char *extra_headers);
bool api_stream_write_event(int fd, int64_t id, const char *event_name, const char *data, size_t data_len);
bool api_stream_write_keepalive(int fd);
bool api_stream_write_retry(int fd, int64_t retry_ms);

// Non-blocking streaming plane process lifecycle and SCM_RIGHTS client handoff.
void stream_client_init(int send_fd);
int stream_plane_run(int recv_fd, pid_t parent_pid);
bool stream_handoff_turn_events(const request *q, const char *account, const char *turn,
                                int64_t after_seq, int64_t turn_seq, int turn_state);
bool stream_handoff_conv_events(const request *q, const char *account, const char *conversation,
                                int64_t after_event_id);
bool stream_handoff_account_events(const request *q, const char *account, int64_t after_id);
