// Push to the apps through Apple's push service and Google's (docs/auth/build-order.md "Outside
// services we can't avoid"; #13233): one function, push_send, behind which either service can be
// swapped by configuration. Push is for security notices only, never a way to log in. Every body
// and Apple's token are auth-push.coax's (proto/auth/push-notice-v1.json); this file only reads
// the settings and moves the bytes.
//
//   Apple (an iPhone or a Mac): POST <url>/3/device/<token> over HTTP/2, with a token signed by
//   our push key (ES256, auth-device-crypto.coax's adc_es256).
//     OA_APNS_KEY          the .p8 file's text (Secret Manager oa-apns-key)
//     OA_APNS_KEY_ID       that key's id (oa-apns-key-id)
//     OA_APNS_TEAM_ID      the developer team (default HQWSG26L43)
//     OA_APNS_TOPIC_IOS    the iPhone app's bundle id (default com.openagents.app)
//     OA_APNS_TOPIC_MAC    the Mac app's (default com.openagents.desktop)
//     OA_APNS_URL, OA_APNS_SANDBOX_URL   the two services (tests point them at a fake)
//   Google (Android): POST <url>/v1/projects/<project>/messages:send, with this service's own
//   Google token from the metadata server.
//     OA_FCM_PROJECT       the Firebase project's id (oa-fcm-project)
//     OA_FCM_URL           the service (tests point it at a fake)
//     OA_FCM_TOKEN         a token to send instead of the metadata server's (tests)
//
// The log names the service and the status: never a device's token, a notice's words or a key.
#include "api.h"
#include "auth-push.generated.h"

#include <curl/curl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

// auth-device-crypto-unit.c
bool adc_es256_c(const unsigned char secret[32], const char *message, size_t n, unsigned char out[64]);

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static size_t drop(char *p, size_t size, size_t count, void *u) { (void)p; (void)u; return size * count; }

// One POST: the status, or 0 when the service wasn't reached.
static long post(const char *url, struct curl_slist *headers, const char *payload, bool http2) {
  CURL *h = curl_easy_init();
  if (!h) return 0;
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, headers);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, payload);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, drop);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 15L);
  curl_easy_setopt(h, CURLOPT_NOSIGNAL, 1L);
  if (http2) curl_easy_setopt(h, CURLOPT_HTTP_VERSION, (long)CURL_HTTP_VERSION_2TLS);   // Apple speaks only HTTP/2; a fake over http stays 1.1
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_easy_cleanup(h);
  return status;
}

// "authorization: bearer <token>" for Apple, signed now: false when the key isn't set up or isn't one.
static bool apns_authorization(char *out, size_t cap) {
  const char *pem = getenv("OA_APNS_KEY"), *kid = getenv("OA_APNS_KEY_ID");
  if (!pem || !*pem || !kid || !*kid) return false;
  unsigned char key[32], sig[64];
  char token[400];
  bool ok = apw_p8_key(str(pem), (coax_v_u8){key, 32});
  int64_t n = ok ? apw_jwt_input(str(kid), str(env_or("OA_APNS_TEAM_ID", "HQWSG26L43")), apw_iat((int64_t)time(NULL)), (coax_v_u8){(uint8_t *)token, sizeof token - 1}) : -1;
  ok = ok && n > 0 && adc_es256_c(key, token, (size_t)n, sig);
  memset(key, 0, sizeof key);
  int64_t m = ok ? apw_jwt_close((coax_v_u8){sig, 64}, n, (coax_v_u8){(uint8_t *)token, sizeof token - 1}) : -1;
  if (m <= 0) { memset(token, 0, sizeof token); return false; }
  token[m] = 0;
  snprintf(out, cap, "authorization: bearer %s", token);
  memset(token, 0, sizeof token);
  return true;
}

static int send_apns(bool sandbox, const char *platform, const char *token, const char *title, const char *body, const char *what, long long notice) {
  char auth[480];
  if (!apns_authorization(auth, sizeof auth)) return -1;
  char payload[3000];
  int64_t n = apw_apns(str(title), str(body), str(what), notice, (coax_v_u8){(uint8_t *)payload, sizeof payload - 1});
  if (n <= 0) return 0;
  payload[n] = 0;
  char url[700], topic[200];
  snprintf(url, sizeof url, "%s/3/device/%s", sandbox ? env_or("OA_APNS_SANDBOX_URL", "https://api.sandbox.push.apple.com") : env_or("OA_APNS_URL", "https://api.push.apple.com"), token);
  snprintf(topic, sizeof topic, "apns-topic: %s", !strcmp(platform, "mac") ? env_or("OA_APNS_TOPIC_MAC", "com.openagents.desktop") : env_or("OA_APNS_TOPIC_IOS", "com.openagents.app"));
  struct curl_slist *headers = curl_slist_append(NULL, "content-type: application/json");
  headers = curl_slist_append(headers, auth);
  headers = curl_slist_append(headers, topic);
  headers = curl_slist_append(headers, "apns-push-type: alert");
  headers = curl_slist_append(headers, "apns-priority: 10");
  long status = post(url, headers, payload, true);
  curl_slist_free_all(headers);
  memset(auth, 0, sizeof auth);
  return (int)status;
}

static int send_fcm(const char *token, const char *title, const char *body, const char *what, long long notice) {
  const char *project = getenv("OA_FCM_PROJECT");
  if (!project || !*project) return -1;
  char bearer[2400] = "", auth[2500];
  const char *given = getenv("OA_FCM_TOKEN");
  if (given && *given) snprintf(bearer, sizeof bearer, "%s", given);
  else if (!inference_google_token(bearer, sizeof bearer)) return 0;
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", bearer);
  memset(bearer, 0, sizeof bearer);
  char payload[3600];
  int64_t n = apw_fcm(str(token), str(title), str(body), str(what), notice, (coax_v_u8){(uint8_t *)payload, sizeof payload - 1});
  if (n <= 0) { memset(auth, 0, sizeof auth); return 0; }
  payload[n] = 0;
  char url[400];
  snprintf(url, sizeof url, "%s/v1/projects/%.120s/messages:send", env_or("OA_FCM_URL", "https://fcm.googleapis.com"), project);
  struct curl_slist *headers = curl_slist_append(NULL, "Content-Type: application/json");
  headers = curl_slist_append(headers, auth);
  long status = post(url, headers, payload, false);
  curl_slist_free_all(headers);
  memset(auth, 0, sizeof auth);
  return (int)status;
}

// One notice to one device. service: apns, apns_sandbox or fcm; platform: ios, mac or android.
// The service's HTTP status (2xx: it took the notice), 0 when it couldn't be reached or the notice
// couldn't be written, -1 when that service isn't set up here.
int push_send(const char *service, const char *platform, const char *token, const char *title, const char *body, const char *what, long long notice) {
  int status;
  if (!strcmp(service, "apns")) status = send_apns(false, platform, token, title, body, what, notice);
  else if (!strcmp(service, "apns_sandbox")) status = send_apns(true, platform, token, title, body, what, notice);
  else if (!strcmp(service, "fcm")) status = send_fcm(token, title, body, what, notice);
  else return -1;
  if (status < 0) return status;
  if (status < 200 || status > 299) api_log("push: %s answered %d", service, status);
  return status;
}
