// RFC 6455 WebSocket: the handshake, frame parsing and frame building.
// Plain bytes in, plain bytes out; no sockets here.
#ifndef OA_VOICE_WS_H
#define OA_VOICE_WS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum { WS_CONT = 0, WS_TEXT = 1, WS_BINARY = 2, WS_CLOSE = 8, WS_PING = 9, WS_PONG = 10 };

void ws_sha1(const uint8_t *data, size_t n, uint8_t out[20]);
size_t ws_base64(const uint8_t *in, size_t n, char *out);          // out needs 4*((n+2)/3)+1
// Returns bytes written to out, or 0 on bad input (whitespace is skipped).
size_t ws_unbase64(const char *in, size_t n, uint8_t *out, size_t cap);

// Server side. `req` is the HTTP request up to and including "\r\n\r\n".
// Writes the 101 response into out; returns its length, or 0 if the request
// is not a valid WebSocket upgrade (the caller sends 400 and closes).
size_t ws_server_handshake(const char *req, size_t n, char *out, size_t cap);
// Client side: the upgrade request for host:port/path with a fixed key.
size_t ws_client_request(const char *host, int port, const char *path, char *out, size_t cap);
bool ws_client_accepts(const char *resp, size_t n);

typedef struct {
  bool fin;
  int opcode;
  const uint8_t *payload;  // unmasked in place
  size_t length;
  size_t consumed;         // header + payload bytes
} ws_frame;

// Parses one frame at the start of buf. Returns 1 with *f filled (payload
// unmasked in place), 0 if more bytes are needed, -1 on a protocol error.
// Frames longer than max_payload are an error.
int ws_parse(uint8_t *buf, size_t n, size_t max_payload, ws_frame *f);
// Builds a final frame; mask=true for a client. Returns header+payload length.
size_t ws_build(uint8_t *out, size_t cap, int opcode, const uint8_t *payload, size_t n, bool mask);
#endif
