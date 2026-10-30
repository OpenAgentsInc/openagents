/* Network.framework owns TCP and TLS together. Included by tls-native.c on
   Apple only; build with -fblocks and Network/Security/CoreFoundation.
   The synchronous scalar API is single-owner, as socket-native is. All
   asynchronous state is serialized on cxtls_queue. Static slots + generation
   checks keep cancelled callbacks from touching a later connection. */
#include <Network/Network.h>
#include <Security/Security.h>
#include <dispatch/dispatch.h>
#include <stdio.h>
#include <stdbool.h>
#include <limits.h>

typedef struct {
  nw_connection_t connection;
  dispatch_semaphore_t changed;
  uint64_t generation;
  bool ready, read_pending, read_done, send_done, eof;
  int64_t failure, verify_failure, received_error, send_error, version;
  size_t received;
  unsigned char receiving[CXTLS_MAX_IN], in[CXTLS_MAX_IN];
  int64_t in_count, out_count;
  unsigned char *out;
} cxtls_slot;
typedef struct { char bytes[CXTLS_MAX_HOST + 1]; } cxtls_name;
static cxtls_slot cxtls_slots[CXSOCK_MAX];
static dispatch_queue_t cxtls_queue;
static dispatch_once_t cxtls_once;
static cxtls_name cxtls_host;
static int cxtls_host_len, cxtls_host_bad;

static void cxtls_init(void) {
  dispatch_once(&cxtls_once, ^{
    cxtls_queue = dispatch_queue_create("com.openagents.coax.tls", DISPATCH_QUEUE_SERIAL);
    for (int i = 0; i < CXSOCK_MAX; i++) cxtls_slots[i].changed = dispatch_semaphore_create(0);
  });
}
static cxtls_slot *cxtls_get(int64_t h) {
  if (h < CXSOCK_MAX || h >= 2 * CXSOCK_MAX) return NULL;
  cxtls_slot *s = &cxtls_slots[h - CXSOCK_MAX];
  return s->connection ? s : NULL;
}
int64_t cxtls_host_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || cxtls_host_len >= CXTLS_MAX_HOST) { cxtls_host_bad = 1; return CXTLS_FULL; }
  cxtls_host.bytes[cxtls_host_len++] = (char)byte;
  return 0;
}
static int64_t cxtls_error(nw_error_t error, bool handshake) {
  if (!error) return 0;
  int code = nw_error_get_error_code(error);
  if (nw_error_get_error_domain(error) == nw_error_domain_tls && code == errSSLPeerProtocolVersion) return CXTLS_VERSION;
  return handshake ? CXTLS_HANDSHAKE : CXTLS_ERROR;
}

#ifdef CXTLS_TEST_TRUST
/* Test binaries only. Import public PEM certificates without a keychain. */
static bool cxtls_test_anchors(SecTrustRef trust) {
  const char *path = getenv("CXTLS_TEST_CA");
  if (!path || !*path) return true;
  FILE *f = fopen(path, "rb"); if (!f) return false;
  unsigned char data[65536]; size_t n = fread(data, 1, sizeof data, f);
  bool valid = n > 0 && n < sizeof data && !ferror(f); fclose(f);
  if (!valid) return false;
  CFDataRef bytes = CFDataCreate(NULL, data, (CFIndex)n);
  CFArrayRef items = NULL;
  SecExternalFormat format = kSecFormatPEMSequence;
  SecExternalItemType type = kSecItemTypeCertificate;
  OSStatus status = SecItemImport(bytes, NULL, &format, &type, 0, NULL, NULL, &items);
  CFRelease(bytes);
  valid = status == errSecSuccess && items && CFArrayGetCount(items) > 0;
  if (valid) valid = SecTrustSetAnchorCertificates(trust, items) == errSecSuccess &&
                     SecTrustSetAnchorCertificatesOnly(trust, true) == errSecSuccess;
  if (items) CFRelease(items);
  return valid;
}
#endif

static int64_t cxtls_verify(sec_trust_t input, cxtls_name name) {
  SecTrustRef trust = sec_trust_copy_ref(input);
  if (!trust) return CXTLS_UNTRUSTED;
  CFStringRef host = CFStringCreateWithCString(NULL, name.bytes, kCFStringEncodingUTF8);
  SecPolicyRef policy = host ? SecPolicyCreateSSL(true, host) : NULL;
  bool ok = policy && SecTrustSetPolicies(trust, policy) == errSecSuccess;
  /* Do not let certificate-controlled AIA/OCSP fetches outlive the caller's
     connect deadline. Supplied intermediates and the system anchors apply. */
  if (ok) ok = SecTrustSetNetworkFetchAllowed(trust, false) == errSecSuccess;
#ifdef CXTLS_TEST_TRUST
  if (ok) ok = cxtls_test_anchors(trust);
#endif
  CFErrorRef error = NULL;
  if (ok) ok = SecTrustEvaluateWithError(trust, &error);
  int64_t result = ok ? 0 : error && CFErrorGetCode(error) == errSecHostNameMismatch ? CXTLS_NAME : CXTLS_UNTRUSTED;
  if (error) CFRelease(error);
  if (policy) CFRelease(policy);
  if (host) CFRelease(host);
  CFRelease(trust);
  return result;
}

/* Wait only for an observed effect: ready, receive completion, send completion.
   A read timeout leaves its one receive pending for the next poll. */
static int64_t cxtls_wait(cxtls_slot *s, int phase, int64_t timeout_ms) {
  int64_t now = cxsock_now_ms();
  if (timeout_ms > INT64_MAX - now) return CXTLS_ERROR;
  int64_t deadline = timeout_ms < 0 ? -1 : now + timeout_ms;
  for (;;) {
    __block bool done; __block int64_t failure;
    dispatch_sync(cxtls_queue, ^{
      done = phase == 0 ? s->ready : phase == 1 ? s->read_done : s->send_done;
      failure = s->failure;
    });
    if (done) return 0;
    if (failure) return failure;
    int64_t left = deadline < 0 ? -1 : deadline - cxsock_now_ms();
    if (deadline >= 0 && left <= 0) return CXTLS_TIMEOUT;
    dispatch_time_t until = left < 0 ? DISPATCH_TIME_FOREVER :
      dispatch_time(DISPATCH_TIME_NOW, left > INT64_MAX / (int64_t)NSEC_PER_MSEC ? INT64_MAX : left * (int64_t)NSEC_PER_MSEC);
    if (dispatch_semaphore_wait(s->changed, until)) return CXTLS_TIMEOUT;
  }
}

static void cxtls_drop(cxtls_slot *s) {
  dispatch_sync(cxtls_queue, ^{
    s->generation++; /* Every old callback checks this before touching a slot. */
    nw_connection_cancel(s->connection);
    nw_release(s->connection); s->connection = NULL;
    s->ready = false;
  });
  free(s->out); s->out = NULL; s->out_count = s->in_count = 0;
}

/* Keep one chunk read ahead. The Coax caller can be computing while the
   platform receives the next frame; a zero-time poll must not start every
   receive from scratch and mistake already-arrived frames for an empty queue. */
static void cxtls_receive_next(cxtls_slot *s, uint64_t generation) {
  if (s->read_pending || s->read_done || s->eof || s->failure || !s->ready) return;
  s->read_pending = true;
  nw_connection_receive(s->connection, 1, CXTLS_MAX_IN, ^(dispatch_data_t data, nw_content_context_t context, bool complete, nw_error_t error) {
    (void)context;
    if (s->generation != generation) return;
    s->received = 0;
    if (data) dispatch_data_apply(data, ^bool(dispatch_data_t region, size_t offset, const void *buffer, size_t size) {
      (void)region;
      if (offset > CXTLS_MAX_IN || size > CXTLS_MAX_IN - offset) return false;
      memcpy(s->receiving + offset, buffer, size); s->received += size; return true;
    });
    s->received_error = cxtls_error(error, false);
    s->eof = complete && !error;
    s->read_pending = false; s->read_done = true;
    dispatch_semaphore_signal(s->changed);
  });
}

int64_t cxtls_connect(int64_t port, int64_t timeout_ms) {
  cxtls_init();
  bool bad = cxtls_host_bad || cxtls_host_len == 0 || port < 1 || port > 65535 || timeout_ms < 0;
  cxtls_host.bytes[cxtls_host_len] = 0;
  cxtls_name name = cxtls_host;
  cxtls_host_len = cxtls_host_bad = 0;
  if (bad || timeout_ms > INT64_MAX - cxsock_now_ms()) return CXTLS_ERROR;
  int64_t deadline = cxsock_now_ms() + timeout_ms;
  int at = -1;
  for (int i = 0; i < CXSOCK_MAX; i++) if (!cxtls_slots[i].connection) { at = i; break; }
  if (at < 0) return CXTLS_FULL;
  cxtls_slot *s = &cxtls_slots[at];
  s->out = malloc(CXTLS_MAX_OUT); if (!s->out) return CXTLS_ERROR;
  char service[8]; snprintf(service, sizeof service, "%d", (int)port);
  __block uint64_t generation;
  dispatch_sync(cxtls_queue, ^{ generation = ++s->generation; });
  nw_parameters_t parameters = nw_parameters_create_secure_tcp(^(nw_protocol_options_t options) {
    sec_protocol_options_t tls = nw_tls_copy_sec_protocol_options(options);
    sec_protocol_options_set_min_tls_protocol_version(tls, tls_protocol_version_TLSv12);
    sec_protocol_options_set_tls_server_name(tls, name.bytes);
    sec_protocol_options_set_peer_authentication_required(tls, true);
    sec_protocol_options_set_verify_block(tls, ^(sec_protocol_metadata_t metadata, sec_trust_t trust, sec_protocol_verify_complete_t complete) {
      (void)metadata;
      int64_t result = cxtls_verify(trust, name);
      dispatch_async(cxtls_queue, ^{
        bool current = s->generation == generation;
        if (current) s->verify_failure = result;
        complete(current && result == 0);
      });
    }, dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0));
    sec_release(tls);
  }, NW_PARAMETERS_DEFAULT_CONFIGURATION);
  nw_endpoint_t endpoint = nw_endpoint_create_host(name.bytes, service);
  s->connection = nw_connection_create(endpoint, parameters);
  nw_release(endpoint); nw_release(parameters);
  if (!s->connection) { free(s->out); s->out = NULL; return CXTLS_ERROR; }
  dispatch_sync(cxtls_queue, ^{
    s->ready = s->read_pending = s->read_done = s->send_done = s->eof = false;
    s->failure = s->verify_failure = s->received_error = s->send_error = s->version = 0;
    s->received = 0; s->in_count = s->out_count = 0;
  });
  nw_connection_set_queue(s->connection, cxtls_queue);
  nw_connection_set_state_changed_handler(s->connection, ^(nw_connection_state_t state, nw_error_t error) {
    if (s->generation != generation) return;
    if (state == nw_connection_state_ready) {
      nw_protocol_definition_t definition = nw_protocol_copy_tls_definition();
      nw_protocol_metadata_t metadata = nw_connection_copy_protocol_metadata(s->connection, definition);
      nw_release(definition);
      if (metadata) {
        sec_protocol_metadata_t tls = nw_tls_copy_sec_protocol_metadata(metadata);
        s->version = sec_protocol_metadata_get_negotiated_tls_protocol_version(tls);
        sec_release(tls); nw_release(metadata);
      }
      if (s->version < 0x0303) s->failure = CXTLS_VERSION;
      else s->ready = true;
      cxtls_receive_next(s, generation);
    } else if (state == nw_connection_state_failed || state == nw_connection_state_waiting) {
      s->failure = s->verify_failure ? s->verify_failure : cxtls_error(error, !s->ready);
      if (!s->failure) s->failure = CXTLS_HANDSHAKE;
    }
    dispatch_semaphore_signal(s->changed);
  });
  nw_connection_start(s->connection);
  int64_t left = deadline - cxsock_now_ms();
  int64_t result = cxtls_wait(s, 0, left < 0 ? 0 : left);
  if (result) { cxtls_drop(s); return result; }
  return CXSOCK_MAX + at;
}

/* Legacy descriptor upgrade is Linux-only. Apple callers use connect above,
   which owns one real TCP/TLS connection; there is no throwaway TCP connect. */
int64_t cxtls_start(int64_t h, int64_t timeout_ms) {
  (void)h; (void)timeout_ms; cxtls_host_len = cxtls_host_bad = 0;
  return CXTLS_UNSUPPORTED;
}
int64_t cxtls_write(int64_t h, int64_t byte) {
  cxtls_slot *s = cxtls_get(h);
  if (!s || byte < 0 || byte > 255) return CXTLS_ERROR;
  if (s->out_count >= CXTLS_MAX_OUT) return CXTLS_FULL;
  s->out[s->out_count++] = (unsigned char)byte;
  return 0;
}
int64_t cxtls_flush(int64_t h, int64_t timeout_ms) {
  cxtls_slot *s = cxtls_get(h); if (!s) return CXTLS_ERROR;
  if (!s->out_count) return 0;
  dispatch_data_t data = dispatch_data_create(s->out, (size_t)s->out_count, cxtls_queue, DISPATCH_DATA_DESTRUCTOR_DEFAULT);
  if (!data) return CXTLS_ERROR;
  s->out_count = 0;
  uint64_t generation = s->generation;
  dispatch_sync(cxtls_queue, ^{
    s->send_done = false; s->send_error = 0;
    nw_connection_send(s->connection, data, NW_CONNECTION_DEFAULT_MESSAGE_CONTEXT, true, ^(nw_error_t error) {
      if (s->generation != generation) return;
      s->send_error = cxtls_error(error, false); s->send_done = true;
      dispatch_semaphore_signal(s->changed);
    });
  });
  dispatch_release(data);
  int64_t result = cxtls_wait(s, 2, timeout_ms);
  __block int64_t error;
  dispatch_sync(cxtls_queue, ^{
    error = s->send_error;
    if (result) { s->failure = result; nw_connection_cancel(s->connection); }
  });
  return result ? result : error;
}
int64_t cxtls_read(int64_t h, int64_t timeout_ms) {
  cxtls_slot *s = cxtls_get(h); if (!s) return CXTLS_ERROR;
  s->in_count = 0;
  uint64_t generation = s->generation;
  dispatch_sync(cxtls_queue, ^{
    cxtls_receive_next(s, generation);
    if (s->eof && !s->read_done) { s->read_done = true; s->received = 0; }
  });
  int64_t result = cxtls_wait(s, 1, timeout_ms); if (result) return result;
  __block int64_t n;
  dispatch_sync(cxtls_queue, ^{
    n = (int64_t)s->received;
    if (n) memcpy(s->in, s->receiving, (size_t)n);
    else n = s->received_error ? s->received_error : s->eof ? CXTLS_END_UNVERIFIED : CXTLS_ERROR;
    if (s->received_error) s->failure = s->received_error;
    s->read_done = false; s->received = 0;
    cxtls_receive_next(s, generation);
  });
  if (n > 0) s->in_count = n;
  return n;
}
int64_t cxtls_byte(int64_t h, int64_t i) {
  cxtls_slot *s = cxtls_get(h);
  return !s || i < 0 || i >= s->in_count ? CXTLS_ERROR : s->in[i];
}
int64_t cxtls_version(int64_t h) {
  cxtls_slot *s = cxtls_get(h); return s ? s->version : CXTLS_ERROR;
}
int64_t cxtls_close(int64_t h) {
  cxtls_slot *s = cxtls_get(h); if (!s) return CXTLS_ERROR;
  /* cancel negotiates protocol shutdown, and delivers outstanding completions
     before its final cancelled event. Generation fencing makes reuse safe. */
  cxtls_drop(s);
  return 0;
}
