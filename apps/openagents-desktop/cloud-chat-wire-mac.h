/* C boundary for coax/web/api/account-chat-cloud-wire.coax (#14093). */
#pragma once
#include <stdbool.h>
#include <stdint.h>
#ifndef COAX_STRING_DEFINED
#define COAX_STRING_DEFINED
typedef struct { const char *data; int64_t count; } coax_string;
#endif
#ifndef COAX_coax_v_u8_DEFINED
#define COAX_coax_v_u8_DEFINED
typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
#endif
#ifndef COAX_coax_v_string_DEFINED
#define COAX_coax_v_string_DEFINED
typedef struct { coax_string *data; int64_t count; } coax_v_string;
#endif
#ifndef COAX_coax_a32_s64_DEFINED
#define COAX_coax_a32_s64_DEFINED
typedef struct { int64_t e[32]; } coax_a32_s64;
#endif
#ifndef COAX_coax_t_CtUsage_DEFINED
#define COAX_coax_t_CtUsage_DEFINED
typedef struct coax_t_CtUsage {
  int64_t input;
  int64_t output;
  int64_t cache_read;
  int64_t cache_write;
  int64_t reasoning;
} coax_t_CtUsage;
#endif
#ifndef COAX_coax_t_CcEvent_DEFINED
#define COAX_coax_t_CcEvent_DEFINED
typedef struct coax_t_CcEvent {
  coax_string conversation;
  coax_string turn;
  int64_t sequence;
  int64_t kind;
  int64_t state;
  coax_string text;
  coax_string tool_id;
  coax_string tool_name;
  coax_string tool_args;
  coax_string diff;
  _Bool failed;
  coax_string approval_status;
  coax_string attachments;
  coax_string compute;
  coax_string model;
  int64_t cost_msat;
  coax_string source_device;
  coax_string workspace;
  _Bool has_usage;
  coax_t_CtUsage usage;
  coax_string stop_reason;
  int64_t created_at_ms;
} coax_t_CcEvent;
typedef struct { coax_t_CcEvent *data; int64_t count; } coax_v_t_CcEvent;
#endif
#ifndef COAX_coax_t_CcSyncReq_DEFINED
#define COAX_coax_t_CcSyncReq_DEFINED
typedef struct coax_t_CcSyncReq {
  coax_string operation;
  coax_string conversation;
  coax_string turn;
  coax_string title;
  _Bool has_pinned;
  _Bool pinned;
  _Bool has_archived;
  _Bool archived;
  coax_string compute;
  coax_string model;
  int64_t cost_msat;
  coax_string source_device;
  coax_string workspace;
  coax_string user_text;
  coax_string attachments;
  int64_t state;
  coax_string output;
  coax_string stop_reason;
  coax_string error;
  _Bool has_usage;
  coax_t_CtUsage usage;
  int64_t event_count;
  coax_a32_s64 event_starts;
  coax_a32_s64 event_ends;
} coax_t_CcSyncReq;
typedef struct { coax_t_CcSyncReq *data; int64_t count; } coax_v_t_CcSyncReq;
#endif
#ifndef COAX_coax_t_CcMetaReq_DEFINED
#define COAX_coax_t_CcMetaReq_DEFINED
typedef struct coax_t_CcMetaReq {
  coax_string operation;
  _Bool has_title;
  coax_string title;
  _Bool has_pinned;
  _Bool pinned;
  _Bool has_archived;
  _Bool archived;
} coax_t_CcMetaReq;
typedef struct { coax_t_CcMetaReq *data; int64_t count; } coax_v_t_CcMetaReq;
#endif

_Bool cc_compute_valid(coax_string compute, int64_t cost_msat);
int64_t cc_event_write(coax_v_t_CcEvent e, coax_v_u8 out);
int64_t cc_sync_write(coax_v_t_CcSyncReq r, coax_v_string events, coax_v_u8 out);
int64_t cc_meta_write(coax_v_t_CcMetaReq r, coax_v_u8 out);
