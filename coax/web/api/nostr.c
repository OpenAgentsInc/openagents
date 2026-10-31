// NIP-98 HTTP auth for the API: "Authorization: Nostr <base64 of a kind 27235
// event>", checked by Coax (nip98.coax, through its checked C
// nip98.generated.c): the event's NIP-01 id and BIP-340 signature, kind
// 27235, created_at within 60 s of now, the "u" tag the exact URL, the
// "method" tag the request's method and a "payload" tag, when present, the
// body's SHA-256. The decisions and refusal words are the former C check's
// (nip98_reference.c), compared by nip98_diff.c.
#include "nostr.h"

#include <stdio.h>
#include <string.h>

#include "nip98.generated.h"

bool nip98_check(const char *authorization, const char *method, const char *url, const void *body, size_t body_len,
                 long long now, nostr_event *ev, char *err, size_t errcap) {
  static uint8_t work[103424], out[1233];
  memset(ev, 0, sizeof *ev);
  coax_string a = {authorization ? authorization : "", authorization ? (int64_t)strlen(authorization) : 0};
  coax_string m = {method ? method : "", method ? (int64_t)strlen(method) : 0};
  coax_string u = {url ? url : "", url ? (int64_t)strlen(url) : 0};
  coax_v_u8 b = {(uint8_t *)(body ? body : ""), body ? (int64_t)body_len : 0};
  coax_t_Nip98 r = nip98_check_coax(a, m, u, b, (int64_t)now, (coax_v_u8){work, sizeof work}, (coax_v_u8){out, sizeof out});
  if (r.error) { coax_string w = nip98_error_text(r.error); snprintf(err, errcap, "%.*s", (int)w.count, w.data); return false; }
  memcpy(ev->id, out, 64); memcpy(ev->pubkey, out + 64, 64);
  ev->created_at = r.created_at; ev->kind = (int)r.kind;
  memcpy(ev->u, out + 128, (size_t)r.u); memcpy(ev->method, out + 1152, (size_t)r.method); memcpy(ev->payload, out + 1168, (size_t)r.payload);
  return true;
}
