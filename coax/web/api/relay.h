// Signing Nostr events and sending them to a relay (relay.c), for publishing a
// project made from a drop (drops.c, rc-publish).
#pragma once
#include <stdbool.h>
#include <stddef.h>

// A new secret key and its x-only public key (hex).
bool nostr_keygen(unsigned char sk[32], char pubhex[65]);
bool nostr_pubkey(const unsigned char sk[32], char pubhex[65]);
// The signed event's JSON (malloc'd) and its id, from tags (a NIP-01 JSON array) and
// content (a NIP-01 JSON string, quotes included), both already serialized.
char *nostr_sign(const unsigned char sk[32], int kind, long long created_at, const char *tags_json,
                 const char *content_json, char id[65]);

// A relay connection (ws:// or wss://). It answers NIP-42 AUTH with sk when the relay asks.
typedef struct relay relay;
relay *relay_open(const char *url, const unsigned char sk[32], char *err, size_t errcap);
// Sends ["EVENT", event] and waits for the relay's OK for id; false with the reason in err.
bool relay_send(relay *r, const char *event_json, const char *id, char *err, size_t errcap);
// Sends ["REQ", sub, filter] and gathers the events until EOSE: a JSON array of them (malloc'd), or NULL
// with the reason in err. filter is a NIP-01 filter object, already serialized.
char *relay_query(relay *r, const char *filter_json, char *err, size_t errcap);
void relay_close(relay *r);

// Sealing a secret: AES-256-GCM under a 32-byte key given as 64 hex digits, with aad bound in.
// sealed is hex(nonce 12 || ciphertext || tag 16).
bool seal(const char *kek_hex, const char *aad, const unsigned char *plain, size_t n, char *sealed, size_t cap);
bool unseal(const char *kek_hex, const char *aad, const char *sealed, unsigned char *plain, size_t n);
