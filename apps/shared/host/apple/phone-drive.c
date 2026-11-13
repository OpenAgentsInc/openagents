// Firm reason: 6 (a socket, a thread and the main queue; the operations' answers are the app's Coax).
// The iPhone app's half of coax drive (The OpenAgents Book, chapter 6; coax/native.md, stage 1; GitHub #13516,
// #13517): the shared hosts' driver (../drive.c) over a UiHost of its own that has no window. The driver's
// socket is served on a thread; each operation it does not answer itself (`tree`, `tap ID`, `type ID TEXT`,
// `state`, the app's `clock data ...`) and each frame `clock +MS` steps go to the main thread, where the app
// holds its state and its UIKit controls. `screenshot` refuses here (it is offscreen-only in drive.c).
// Two transports (chapter 6, "Who may drive a phone"):
//   SOCKET a path      a Unix socket: the simulator, whose app shares the Mac's file system
//   tcp:PORT           TCP on the phone's own network, for a physical iPhone. A driver is admitted only from
//                      an address on one of the phone's own subnets (Wi-Fi or USB, never the cellular one, or
//                      loopback), and only when its first line is `attach TOKEN` with the token the app was
//                      launched with (-OADriveToken, 32 or more characters). It is answered `attached`, or
//                      `refused: ...` and closed. One driver at a time.
//   coax/web/tool.sh drive native SOCKET --ops FILE                        (-OADrive SOCKET)
//   coax/web/tool.sh drive native tcp:HOST:PORT --token TOKEN --ops FILE   (-OADrive tcp:PORT -OADriveToken TOKEN)
#include <arpa/inet.h>
#include <dispatch/dispatch.h>
#include <fcntl.h>
#include <ifaddrs.h>
#include <net/if.h>
#include <netinet/in.h>
#include <poll.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include "../linux/ui-host.h"
#include "phone-drive.h"
#include "../drive.h"

struct UiHost { UiApp app; UiDrive drive; int offscreen; uint32_t w, rows; int stale; };
static UiHost phone;
static PhoneDriveAnswer phone_answer;
static PhoneDriveStep phone_step;
#define PHONE_TOKEN_MIN 32
#define PHONE_TOKEN_MAX 128
static char phone_token[PHONE_TOKEN_MAX + 1];   // empty on a Unix socket, which admits whoever can reach the path

// What drive.c asks of a host: a frame (`clock +MS`), and pictures, which this host doesn't take.
typedef struct { float dt; } PhoneFrame;
static void phone_frame(void *ctx) { phone_step(((PhoneFrame *)ctx)->dt); }
static void ui_fill(UiHost *h, float dt) { (void)h; PhoneFrame f = {dt}; dispatch_sync_f(dispatch_get_main_queue(), &f, phone_frame); }
int ui_shot_frame(UiHost *h, uint32_t w, uint32_t rows, float dt) { (void)h; (void)w; (void)rows; (void)dt; return 0; }
int ui_shot_write(UiHost *h, const char *png) { (void)h; (void)png; return 0; }

typedef struct { const char *op; char *out; int cap; int n; } PhoneOp;
static void phone_op(void *ctx) { PhoneOp *o = ctx; o->n = phone_answer(o->op, o->out, o->cap); }
static int phone_drive_app(void *user, const char *op, char *out, int cap) {
  (void)user;
  PhoneOp o = {op, out, cap, -1};
  dispatch_sync_f(dispatch_get_main_queue(), &o, phone_op);
  return o.n;
}

// ---- who may drive a phone over TCP

// The address as 16 bytes of IPv6 (an IPv4 one mapped), so both families compare the same way.
static int phone_addr16(const struct sockaddr *sa, unsigned char out[16]) {
  if (sa->sa_family == AF_INET6) { memcpy(out, &((const struct sockaddr_in6 *)sa)->sin6_addr, 16); return 1; }
  if (sa->sa_family == AF_INET) {
    memset(out, 0, 10); out[10] = 0xff; out[11] = 0xff;
    memcpy(out + 12, &((const struct sockaddr_in *)sa)->sin_addr, 4); return 1;
  }
  return 0;
}
static int phone_is_v4(const unsigned char a[16]) {
  static const unsigned char head[12] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff};
  return !memcmp(a, head, 12);
}
// 1 when the peer is loopback, or on the subnet of one of the phone's own interfaces that is up and not
// cellular (pdp_ip*): the Wi-Fi network the phone is on, or the USB link to the Mac.
static int phone_local_peer(const struct sockaddr *peer) {
  unsigned char p[16];
  if (!phone_addr16(peer, p)) return 0;
  static const unsigned char lo6[16] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};
  if (!memcmp(p, lo6, 16) || (phone_is_v4(p) && p[12] == 127)) return 1;
  struct ifaddrs *list = NULL;
  if (getifaddrs(&list) != 0) return 0;
  int local = 0;
  for (struct ifaddrs *i = list; i && !local; i = i->ifa_next) {
    if (!i->ifa_addr || !i->ifa_netmask || !(i->ifa_flags & IFF_UP) || !strncmp(i->ifa_name, "pdp_ip", 6)) continue;
    unsigned char a[16], m[16];
    if (!phone_addr16(i->ifa_addr, a) || !phone_addr16(i->ifa_netmask, m)) continue;
    if (phone_is_v4(a) != phone_is_v4(p)) continue;
    if (phone_is_v4(m)) memset(m, 0xff, 12);   // an IPv4 mask covers only the last four bytes
    int same = 1;
    for (int k = 0; k < 16; k++) if ((a[k] & m[k]) != (p[k] & m[k])) { same = 0; break; }
    local = same;
  }
  freeifaddrs(list);
  return local;
}
static void phone_say(int fd, const char *text) {
  char head[24]; int k = snprintf(head, sizeof head, "%d\n", (int)strlen(text));
  if (write(fd, head, (size_t)k) != k) return;
  ssize_t w = write(fd, text, strlen(text)); (void)w;
}
// The token, compared in constant time over its whole length.
static int phone_token_matches(const char *got, size_t n) {
  size_t want = strlen(phone_token);
  unsigned diff = (unsigned)(n ^ want);
  for (size_t k = 0; k < want; k++) diff |= (unsigned char)phone_token[k] ^ (unsigned char)(k < n ? got[k] : 0);
  return diff == 0;
}
// drive.c's UI_DRIVE_ADMIT: the connection is the driver's when it comes from the local network and its
// first line, within five seconds, is `attach TOKEN`.
static int phone_admit(UiHost *h, int fd) {
  (void)h;
  if (!phone_token[0]) return 1;   // a Unix socket
  struct sockaddr_storage peer; socklen_t len = sizeof peer;
  if (getpeername(fd, (struct sockaddr *)&peer, &len) != 0 || !phone_local_peer((struct sockaddr *)&peer)) {
    phone_say(fd, "refused: only a driver on this phone's own network may attach");
    return 0;
  }
  char line[PHONE_TOKEN_MAX + 16]; size_t n = 0;
  struct timespec start; clock_gettime(CLOCK_MONOTONIC, &start);
  for (;;) {
    struct timespec now; clock_gettime(CLOCK_MONOTONIC, &now);
    int left = 5000 - (int)((now.tv_sec - start.tv_sec) * 1000 + (now.tv_nsec - start.tv_nsec) / 1000000);
    struct pollfd p = {.fd = fd, .events = POLLIN};
    if (left <= 0 || poll(&p, 1, left) <= 0) { phone_say(fd, "refused: no attach line"); return 0; }
    char c; if (read(fd, &c, 1) != 1) return 0;
    if (c == '\n') break;
    if (n >= sizeof line - 1) { phone_say(fd, "refused: the attach line is too long"); return 0; }
    line[n++] = c;
  }
  if (n < 7 || memcmp(line, "attach ", 7) || !phone_token_matches(line + 7, n - 7)) { phone_say(fd, "refused: wrong token"); return 0; }
  phone_say(fd, "attached");
  return 1;
}
#define UI_DRIVE_ADMIT phone_admit

#include "../drive.c"

static int phone_listen_tcp(int port) {
  int fd = socket(AF_INET6, SOCK_STREAM, 0);
  if (fd < 0) return 0;
  int zero = 0, one = 1;
  setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, &zero, sizeof zero);   // IPv4 peers too, as mapped addresses
  setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in6 addr = {.sin6_family = AF_INET6, .sin6_port = htons((uint16_t)port), .sin6_addr = in6addr_any};
  if (bind(fd, (struct sockaddr *)&addr, sizeof addr) < 0 || listen(fd, 1) < 0) { close(fd); return 0; }
  fcntl(fd, F_SETFL, fcntl(fd, F_GETFL, 0) | O_NONBLOCK);
  phone.drive.listen_fd = fd; phone.drive.conn_fd = -1; phone.drive.line_n = 0;
  return 1;
}

static void *phone_serve(void *arg) { (void)arg; ui_drive_serve(&phone); ui_drive_close(&phone); return NULL; }

int phone_drive_start(const char *where, const char *token, PhoneDriveAnswer answer, PhoneDriveStep step) {
  if (!where || !answer || !step || phone.drive.listen_fd > 0) return 0;
  phone_answer = answer; phone_step = step;
  phone.app.drive = phone_drive_app;
  phone.drive.conn_fd = -1;
  ui_agent_clock(&phone, 1);   // the app's clock moves only when `clock` moves it
  if (!strncmp(where, "tcp:", 4)) {
    int port = atoi(where + 4);
    size_t tn = token ? strlen(token) : 0;
    if (port < 1 || port > 65535 || tn < PHONE_TOKEN_MIN || tn > PHONE_TOKEN_MAX) return 0;   // no token, no listener
    memcpy(phone_token, token, tn + 1);
    if (!phone_listen_tcp(port)) return 0;
  } else if (!ui_drive_listen(&phone, where)) return 0;
  pthread_t t;
  if (pthread_create(&t, NULL, phone_serve, NULL) != 0) return 0;
  pthread_detach(t);
  return 1;
}
