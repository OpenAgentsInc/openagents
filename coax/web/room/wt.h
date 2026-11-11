/* The room's WebTransport server (#13175): QUIC (ngtcp2), HTTP/3 (nghttp3) and TLS 1.3 (picotls,
   minicrypto) from coax/web/room/third_party/, and the WebTransport-over-HTTP/3 session layer on
   top of them, which is ours (wt.c): extended CONNECT with :protocol webtransport (RFC 9220), the
   SETTINGS that enable HTTP datagrams and WebTransport, the capsule protocol on the CONNECT stream,
   and routing datagrams (RFC 9221, RFC 9297 quarter stream ids) and WebTransport streams (0x41
   bidirectional, 0x54 unidirectional) to their session.
   One UDP socket, polled by the host's own loop: nothing here starts a thread or waits.
     wt_server *s = wt_listen("0.0.0.0", 443, "cert.pem", "key.pem", &events, user);
     poll wt_fd(s) for POLLIN and wake by wt_timeout_ms(s); then wt_read(s) and wt_flush(s). */
#ifndef COAX_ROOM_WT_H
#define COAX_ROOM_WT_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define WT_SESSIONS 512          /* sessions at once, every connection together */
#define WT_DATAGRAM_MAX 1200     /* the most one datagram's payload may hold (what the session says) */

typedef struct wt_server wt_server;
typedef struct {
  /* A session asks to open: its :path and :authority. True accepts it (200), false refuses it (404). */
  bool (*session)(void *user, int sid, const char *path, const char *authority, const char *origin);
  /* A datagram heard on session sid. */
  void (*datagram)(void *user, int sid, const uint8_t *data, size_t n);
  /* Bytes on one of session sid's WebTransport streams (the peer's or ours), in order; fin: it ended. */
  void (*stream)(void *user, int sid, int64_t stream, const uint8_t *data, size_t n, bool fin);
  /* Session sid ended (its CONNECT stream or its connection closed). Its id may be reused after. */
  void (*closed)(void *user, int sid);
} wt_events;

typedef struct {
  int64_t datagrams_in, datagrams_out, datagrams_dropped;   /* dropped: never sent (too big, queue full or stale) */
  int64_t datagrams_lost;                                   /* sent, and the peer never acknowledged them */
  int64_t packets_in, packets_out, bytes_in, bytes_out;
  int64_t connections, sessions;
} wt_stats;

/* Listens on UDP address:port with a PEM certificate chain and its PKCS#8 private key (ECDSA P-256:
   Chrome's serverCertificateHashes takes only that, valid for under 14 days). NULL on failure. */
wt_server *wt_listen(const char *address, int port, const char *cert_pem, const char *key_pem, const wt_events *events, void *user);
int wt_fd(const wt_server *s);
/* Every datagram waiting on the socket, read and handled. */
void wt_read(wt_server *s);
/* Every connection's timers and waiting bytes: call after wt_read and after sending. */
void wt_flush(wt_server *s);
/* Milliseconds until the next timer (0: now; capped at 1000; 1 while datagrams wait on the window). */
int wt_timeout_ms(const wt_server *s);
/* One datagram to session sid, sent at the next flush: 0, or -1 (no such session, over
   WT_DATAGRAM_MAX, or the session's queue full; counted as dropped). */
int wt_send_datagram(wt_server *s, int sid, const uint8_t *data, size_t n);
/* Datagrams waiting for session sid, not yet sent. */
int wt_datagrams_waiting(const wt_server *s, int sid);
/* Drops session sid's datagrams that wait unsent (the stale-state rule: a newer state replaces them). */
int wt_drop_waiting(wt_server *s, int sid);
/* A new stream of ours in session sid (bidirectional or not): its id, or -1. */
int64_t wt_open_stream(wt_server *s, int sid, bool bidi);
/* Bytes onto stream (ours, or the peer's bidirectional one), kept until acknowledged: 0, or -1. */
int wt_stream_send(wt_server *s, int sid, int64_t stream, const uint8_t *data, size_t n, bool fin);
/* Bytes put on session sid's streams and not yet acknowledged. */
size_t wt_stream_waiting(const wt_server *s, int sid);
/* Ends session sid: a CLOSE_WEBTRANSPORT_SESSION capsule and the CONNECT stream's end. */
void wt_close(wt_server *s, int sid, uint32_t code);
/* Session sid's smoothed round trip (ms), or -1. */
int wt_rtt_ms(const wt_server *s, int sid);
/* Datagrams session sid's connection sent that the peer never acknowledged (lost), or -1. */
int64_t wt_session_lost(const wt_server *s, int sid);
wt_stats wt_get_stats(const wt_server *s);
void wt_free(wt_server *s);
#endif
