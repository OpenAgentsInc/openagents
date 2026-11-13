// Firm reason: 6 (a Unix socket, a thread and the main queue; the operations' answers are the app's Coax).
// The iPhone app's half of coax drive (The OpenAgents Book, chapter 6; coax/native.md, stage 1; GitHub #13516):
// the shared hosts' driver (../drive.c) over a UiHost of its own that has no window. The driver's socket is
// served on a thread; each operation it does not answer itself (`tree`, `tap ID`, `type ID TEXT`, `state`,
// the app's `clock data ...`) and each frame `clock +MS` steps go to the main thread, where the app holds
// its state and its UIKit controls. `screenshot` refuses here (it is offscreen-only in drive.c).
//   coax/web/tool.sh drive native SOCKET --ops FILE        (the app launched with -OADrive SOCKET)
#include <dispatch/dispatch.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "../linux/ui-host.h"
#include "phone-drive.h"
#include "../drive.h"

struct UiHost { UiApp app; UiDrive drive; int offscreen; uint32_t w, rows; int stale; };
static UiHost phone;
static PhoneDriveAnswer phone_answer;
static PhoneDriveStep phone_step;

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

#include "../drive.c"

static void *phone_serve(void *arg) { (void)arg; ui_drive_serve(&phone); ui_drive_close(&phone); return NULL; }

int phone_drive_start(const char *path, PhoneDriveAnswer answer, PhoneDriveStep step) {
  if (!answer || !step || phone.drive.listen_fd > 0) return 0;
  phone_answer = answer; phone_step = step;
  phone.app.drive = phone_drive_app;
  phone.drive.conn_fd = -1;
  ui_agent_clock(&phone, 1);   // the app's clock moves only when `clock` moves it
  if (!ui_drive_listen(&phone, path)) return 0;
  pthread_t t;
  if (pthread_create(&t, NULL, phone_serve, NULL) != 0) return 0;
  pthread_detach(t);
  return 1;
}
