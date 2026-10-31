// NIP-98 HTTP auth for the API (nostr.c, checked by nip98.coax).
#pragma once
#include <stdbool.h>
#include <stddef.h>

typedef struct {
  char id[65], pubkey[65];
  long long created_at;
  int kind;
  char u[1024], method[16], payload[65];   // NIP-98's tags, when present
} nostr_event;

// "Nostr <base64>" for this method and absolute URL, now in Unix seconds.
bool nip98_check(const char *authorization, const char *method, const char *url, const void *body, size_t body_len,
                 long long now, nostr_event *ev, char *err, size_t errcap);
