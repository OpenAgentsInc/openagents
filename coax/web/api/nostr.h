// Nostr events and NIP-98 HTTP auth for the API (nostr.c).
#pragma once
#include <stdbool.h>
#include <stddef.h>

typedef struct {
  char id[65], pubkey[65];
  long long created_at;
  int kind;
  char u[1024], method[16], payload[65];   // NIP-98's tags, when present
} nostr_event;

// A signed event's JSON: its id is its NIP-01 hash and its BIP-340 signature verifies.
bool nostr_verify_event(const char *json, size_t len, nostr_event *ev, char *err, size_t errcap);
// "Nostr <base64>" for this method and absolute URL, now in Unix seconds.
bool nip98_check(const char *authorization, const char *method, const char *url, const void *body, size_t body_len,
                 long long now, nostr_event *ev, char *err, size_t errcap);
