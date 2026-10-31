# Browser HTTP effects

The browser host sends same-origin bytes on behalf of Coax. Coax owns request
content, admission, retry/cadence policy, decoding and the meaning of a response.
The existing `coaxial_http` import remains available. The bounded and cancel
imports add an optional lifecycle contract for consumers such as game decisions.

```c
int32_t coaxial_http(uint32_t url, uint32_t url_len,
    uint32_t body, uint32_t body_len, int32_t flags);
int32_t coaxial_http_bounded(uint32_t url, uint32_t url_len,
    uint32_t body, uint32_t body_len, int32_t flags, int64_t timeout_ms);
int32_t coaxial_http_cancel(int32_t id);
int64_t coaxial_now_ms(void);
```

Pointers address `env.memory`. The caller supplies valid spans. The host copies
URL/body bytes before returning; later input reuse or memory growth cannot
alter an accepted request. A URL must begin with `/`, stay on the current origin
after browser URL normalization, and not begin with `//`.

Flags retain their existing bits: 1 POST (otherwise GET), 2 beacon, 4 answer
wanted, 8 `X-Verse-Player`, 16 first body line is `Authorization` and remaining
bytes are the body. POST uses JSON content type. Requests include same-origin
cookies. A guest playtest player header does not authenticate an account.

Answer-bearing requests share eight slots, IDs 0–7. Refusal returns -1 without
a callback. An answer calls `coaxial_message(state, 512 + id, length)` with
UTF-8 `STATUS\nBODY`. Network failure is `0\n`. Legacy oversized responses retain
their previous status-only inbox behavior. Beacon/fire-and-forget behavior is
unchanged and has no response callback.

The bounded import requires flag 4, refuses flag 2, and accepts integer deadlines
1–2147483647 milliseconds. It streams at most the module inbox capacity,
including the status header, before decoding. An overflow or expired deadline
produces exactly one `0\n` response and aborts the fetch. Malformed UTF-8 is also
refused, so replacement characters cannot expand the response beyond the cap.
It never delivers a truncated successful body. The receiver must still enforce
its smaller decoder limits and validate all bytes before using an answer.

Cancellation returns 1 for an owned pending ID, otherwise 0. It frees the slot
and suppresses all later callbacks, including a body read already in progress.
It emits no terminal reply; Coax closes its own ticket. Starting an engine
replacement, installing the accepted replacement, or leaving the page fences
all response-bearing requests. Old promises cannot enter a successor engine or
a reused slot. Beacons may outlive the page as before.

`coaxial_now_ms` returns `floor(performance.now())` as an i64. Use this monotonic
effect for expiry, including after background suspension. Frame delta and wall
date are unsuitable. A delayed browser timer does not extend a Coax ticket's
authority: recheck current monotonic time at application.

Abort and expiry revoke local ownership; they do not prove a server or Pylon
stopped work. Propagating cancellation beyond the HTTP connection is a separate
service contract.

Focused checks:

```shell
bun test coax/web/http-requests.test.ts
psionic/tests/check_game_decision.sh --mutate --jobs 4
```
