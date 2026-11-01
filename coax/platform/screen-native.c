/* The compositor's faces to the machine (os/compositor.coax): one screen through DRM/KMS dumb buffers,
   and keyboards and mice through evdev, grabbed so keys reach the compositor and not the console
   under it. Effects only: what to draw and what a key means are Coax. Modelled on verse/kms.
   Linux only, and the screen needs a free VT (no other program may hold the display). Input can also
   come from a file of struct input_event records, for checks without devices. See screen-native.h. */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "screen-native.h"
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#if defined(__linux__)
#include <linux/input.h>
#include <linux/kd.h>
#include <linux/vt.h>
#include <poll.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

typedef struct { uint32_t handle, pitch, fb; uint64_t size; uint32_t *map; } ScrBuf;
static int scr_fd = -1, scr_w, scr_h, scr_back, scr_flip_pending;
static uint32_t scr_crtc, scr_conn;
static drmModeModeInfo scr_mode;
static drmModeCrtc *scr_saved;
static ScrBuf scr_buf[2];
/* The VT the session runs on, when it has one: switches away and back are this process's to grant. */
static int vt_fd = -1, vt_num, vt_kb = -1, vt_active = 1;
static volatile sig_atomic_t vt_rel, vt_acq;

static int scr_make(ScrBuf *b) {
  struct drm_mode_create_dumb c = {.width = (uint32_t)scr_w, .height = (uint32_t)scr_h, .bpp = 32};
  if (drmIoctl(scr_fd, DRM_IOCTL_MODE_CREATE_DUMB, &c) < 0) return -1;
  b->handle = c.handle; b->pitch = c.pitch; b->size = c.size;
  if (drmModeAddFB(scr_fd, (uint32_t)scr_w, (uint32_t)scr_h, 24, 32, b->pitch, b->handle, &b->fb) < 0) return -1;
  struct drm_mode_map_dumb m = {.handle = b->handle};
  if (drmIoctl(scr_fd, DRM_IOCTL_MODE_MAP_DUMB, &m) < 0) return -1;
  b->map = mmap(0, b->size, PROT_READ | PROT_WRITE, MAP_SHARED, scr_fd, (off_t)m.offset);
  if (b->map == MAP_FAILED) { b->map = NULL; return -1; }
  memset(b->map, 0, b->size);
  return 0;
}
static void scr_free(ScrBuf *b) {
  if (b->map) munmap(b->map, b->size);
  if (b->fb) drmModeRmFB(scr_fd, b->fb);
  struct drm_mode_destroy_dumb d = {.handle = b->handle};
  if (b->handle) drmIoctl(scr_fd, DRM_IOCTL_MODE_DESTROY_DUMB, &d);
  memset(b, 0, sizeof *b);
}
static void scr_on_flip(int fd, unsigned seq, unsigned s, unsigned us, void *data) { (void)fd; (void)seq; (void)s; (void)us; (void)data; scr_flip_pending = 0; }

/* Takes the first connected screen at its preferred mode: 0, or a negative step that failed
   (-1 no device, -2 another program holds it, -3 no screen, -4 no CRTC, -5 buffers, -6 mode set). */
int64_t cxscr_open(void) {
  const char *card = getenv("CODEROS_COMPOSITOR_CARD");
  if (card) scr_fd = open(card, O_RDWR | O_CLOEXEC);
  for (int i = 0; scr_fd < 0 && i < 8; i++) {
    char path[32]; snprintf(path, sizeof path, "/dev/dri/card%d", i);
    int fd = open(path, O_RDWR | O_CLOEXEC); if (fd < 0) continue;
    drmModeRes *r = drmModeGetResources(fd);
    int ok = r && r->count_connectors > 0; if (r) drmModeFreeResources(r);
    if (ok) scr_fd = fd; else close(fd);
  }
  if (scr_fd < 0) return -1;
  for (int i = 0; i < 50 && drmSetMaster(scr_fd) < 0 && errno == EBUSY; i++) usleep(100000);
  if (drmSetMaster(scr_fd) < 0 && !drmIsMaster(scr_fd)) { close(scr_fd); scr_fd = -1; return -2; }
  drmModeRes *res = drmModeGetResources(scr_fd);
  drmModeConnector *conn = NULL;
  for (int i = 0; res && i < res->count_connectors; i++) {
    drmModeConnector *c = drmModeGetConnector(scr_fd, res->connectors[i]);
    if (c && c->connection == DRM_MODE_CONNECTED && c->count_modes > 0) { conn = c; break; }
    if (c) drmModeFreeConnector(c);
  }
  if (!conn) { if (res) drmModeFreeResources(res); return -3; }
  scr_mode = conn->modes[0];
  for (int i = 0; i < conn->count_modes; i++) if (conn->modes[i].type & DRM_MODE_TYPE_PREFERRED) { scr_mode = conn->modes[i]; break; }
  drmModeEncoder *enc = conn->encoder_id ? drmModeGetEncoder(scr_fd, conn->encoder_id) : NULL;
  if (enc && enc->crtc_id) scr_crtc = enc->crtc_id;
  for (int i = 0; !scr_crtc && i < conn->count_encoders; i++) {
    drmModeEncoder *e = drmModeGetEncoder(scr_fd, conn->encoders[i]);
    for (int j = 0; e && j < res->count_crtcs; j++) if (e->possible_crtcs & (1u << j)) { scr_crtc = res->crtcs[j]; break; }
    if (e) drmModeFreeEncoder(e);
  }
  if (enc) drmModeFreeEncoder(enc);
  scr_conn = conn->connector_id; drmModeFreeConnector(conn); drmModeFreeResources(res);
  if (!scr_crtc) return -4;
  scr_saved = drmModeGetCrtc(scr_fd, scr_crtc);
  scr_w = scr_mode.hdisplay; scr_h = scr_mode.vdisplay;
  if (scr_make(&scr_buf[0]) < 0 || scr_make(&scr_buf[1]) < 0) return -5;
  if (drmModeSetCrtc(scr_fd, scr_crtc, scr_buf[0].fb, 0, 0, &scr_conn, 1, &scr_mode) < 0) return -6;
  drmModeSetCursor(scr_fd, scr_crtc, 0, 0, 0);   /* a cursor another session left on the CRTC: off (the pointer is drawn in the frame) */
  scr_back = 1;
  return 0;
}
int64_t cxscr_width(void) { return scr_w; }
int64_t cxscr_height(void) { return scr_h; }
int64_t cxscr_refresh(void) { return scr_mode.vrefresh; }
/* Shows rgb (w*h*3 bytes) on the screen at the next vertical blank, copying only rows y0..y1 into
   the back buffer (the rest of it already holds them): 0, -1 when the last one has not been shown
   yet (nothing changes), -2 on an error. */
int64_t cxscr_present(coax_v_u8 rgb, int64_t y0, int64_t y1) {
  if (scr_fd < 0) return -2;
  if (!vt_active) return -1;   /* another VT is showing: the frame waits for this one */
  if (scr_flip_pending) {
    struct pollfd p = {scr_fd, POLLIN, 0};
    if (poll(&p, 1, 0) > 0) { drmEventContext ev = {.version = 2, .page_flip_handler = scr_on_flip}; drmHandleEvent(scr_fd, &ev); }
    if (scr_flip_pending) return -1;
  }
  if (rgb.count < (int64_t)scr_w * scr_h * 3) return -2;
  if (y0 < 0) y0 = 0; if (y1 > scr_h) y1 = scr_h;
  ScrBuf *b = &scr_buf[scr_back];
  const uint8_t *s = rgb.data;
  uint32_t row[8192];
  int w = scr_w < 8192 ? scr_w : 8192;
  for (int y = (int)y0; y < (int)y1; y++) {   // whole rows: the buffer is write-combined
    const uint8_t *p = s + (size_t)y * scr_w * 3;
    for (int x = 0; x < w; x++) row[x] = ((uint32_t)p[3 * x] << 16) | ((uint32_t)p[3 * x + 1] << 8) | p[3 * x + 2];
    memcpy((char *)b->map + (size_t)y * b->pitch, row, (size_t)w * 4);
  }
  if (drmModePageFlip(scr_fd, scr_crtc, b->fb, DRM_MODE_PAGE_FLIP_EVENT, NULL) < 0) return -2;
  scr_flip_pending = 1; scr_back ^= 1;
  return 0;
}
/* Puts back what the screen showed before and lets go of it. */
int64_t cxscr_close(void) {
  if (scr_fd < 0) return 0;
  if (scr_saved) { drmModeSetCrtc(scr_fd, scr_saved->crtc_id, scr_saved->buffer_id, scr_saved->x, scr_saved->y, &scr_conn, 1, &scr_saved->mode); drmModeFreeCrtc(scr_saved); scr_saved = NULL; }
  scr_free(&scr_buf[0]); scr_free(&scr_buf[1]);
  drmDropMaster(scr_fd); close(scr_fd); scr_fd = -1;
  return 0;
}

#define IN_MAX 32
static int in_fd[IN_MAX], in_n, in_file;
static int in_bit(const unsigned long *b, int i) { return (b[i / (8 * sizeof(long))] >> (i % (8 * sizeof(long)))) & 1; }
/* Opens every keyboard (a device with a space bar) and mouse (relative x and y and a left button)
   under /dev/input and grabs it: the count. With path set, reads input_event records from that file
   instead, grabbing nothing. */
int64_t cxin_open(coax_v_u8 path) {
  if (path.count > 0) {
    char p[1024]; if (path.count >= 1024) return -ENAMETOOLONG;
    memcpy(p, path.data, (size_t)path.count); p[path.count] = 0;
    int fd = open(p, O_RDONLY | O_CLOEXEC); if (fd < 0) return -(int64_t)errno;
    in_fd[0] = fd; in_n = 1; in_file = 1; return 1;
  }
  DIR *d = opendir("/dev/input"); if (!d) return -(int64_t)errno;
  struct dirent *e;
  while ((e = readdir(d)) && in_n < IN_MAX) {
    if (strncmp(e->d_name, "event", 5) != 0) continue;
    char p[300]; snprintf(p, sizeof p, "/dev/input/%s", e->d_name);
    int fd = open(p, O_RDONLY | O_NONBLOCK | O_CLOEXEC); if (fd < 0) continue;
    unsigned long keys[KEY_MAX / (8 * sizeof(long)) + 1] = {0}, rel[REL_MAX / (8 * sizeof(long)) + 1] = {0};
    ioctl(fd, EVIOCGBIT(EV_KEY, sizeof keys), keys); ioctl(fd, EVIOCGBIT(EV_REL, sizeof rel), rel);
    int keyboard = in_bit(keys, KEY_SPACE), mouse = in_bit(rel, REL_X) && in_bit(rel, REL_Y) && in_bit(keys, BTN_LEFT);
    if (!keyboard && !mouse) { close(fd); continue; }
    ioctl(fd, EVIOCGRAB, 1);
    in_fd[in_n++] = fd;
  }
  closedir(d);
  return in_n;
}
/* Reads the events ready into the face's own list: the count, 0 for none, -1 when a file source
   is at its end. cxin_type / cxin_code / cxin_value(i) read the list. */
#define IN_EV 1024
static struct input_event in_ev[IN_EV];
static int in_got, in_held;
static struct input_event in_next;
static double in_t0, in_e0;
static double in_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + t.tv_nsec / 1e9; }
int64_t cxin_read(void) {
  in_got = 0;
  if (in_file) {   // a file plays out on its own clock: each record when its time comes
    while (in_got < IN_EV) {
      if (!in_held) {
        if (read(in_fd[0], &in_next, sizeof in_next) != (ssize_t)sizeof in_next) return in_got > 0 ? in_got : -1;
        in_held = 1;
        double e = (double)in_next.input_event_sec + in_next.input_event_usec / 1e6;
        if (in_t0 == 0) { in_t0 = in_now(); in_e0 = e; }
      }
      double e = (double)in_next.input_event_sec + in_next.input_event_usec / 1e6;
      if (e - in_e0 > in_now() - in_t0) break;
      in_ev[in_got++] = in_next; in_held = 0;
    }
    return in_got;
  }
  if (!vt_active) {   /* another VT is showing and its session reads these keys: drop them */
    struct input_event junk[64];
    for (int i = 0; i < in_n; i++) while (read(in_fd[i], junk, sizeof junk) > 0) {}
    return 0;
  }
  for (int i = 0; i < in_n; i++) {
    ssize_t n;
    while (in_got + 64 <= IN_EV && (n = read(in_fd[i], in_ev + in_got, 64 * sizeof in_ev[0])) > 0) in_got += (int)(n / (ssize_t)sizeof in_ev[0]);
  }
  return in_got;
}
int64_t cxin_type(int64_t i) { return (i >= 0 && i < in_got) ? in_ev[i].type : -1; }
int64_t cxin_code(int64_t i) { return (i >= 0 && i < in_got) ? in_ev[i].code : -1; }
int64_t cxin_value(int64_t i) { return (i >= 0 && i < in_got) ? in_ev[i].value : 0; }
int64_t cxin_close(void) {
  for (int i = 0; i < in_n; i++) { if (!in_file) ioctl(in_fd[i], EVIOCGRAB, 0); close(in_fd[i]); }
  in_n = 0; in_file = 0; in_held = 0; in_t0 = 0;
  return 0;
}

static void vt_on_rel(int sig) { (void)sig; vt_rel = 1; }
static void vt_on_acq(int sig) { (void)sig; vt_acq = 1; }
/* Takes the VT the session runs on ($CODEROS_COMPOSITOR_VT, a /dev/ttyN, else the controlling
   terminal): the console's text and keyboard off, and switches handed to this process, which the
   kernel asks with SIGUSR1 (let go) and SIGUSR2 (yours again). Call it before cxscr_open. The VT's
   number; 0 when there is no VT (a test run), and then switches are not this process's business. */
int64_t cxvt_open(void) {
  const char *p = getenv("CODEROS_COMPOSITOR_VT"); if (!p || !*p) p = "/dev/tty";
  int fd = open(p, O_RDWR | O_CLOEXEC | O_NOCTTY); if (fd < 0) return 0;
  unsigned dev = 0; struct stat sb;
  if (fstat(fd, &sb) == 0 && major(sb.st_rdev) == 4) dev = (unsigned)sb.st_rdev;
  else if (ioctl(fd, TIOCGDEV, &dev) < 0) dev = 0;
  struct vt_stat st;
  if (major(dev) != 4 || minor(dev) < 1 || minor(dev) > 63 || ioctl(fd, VT_GETSTATE, &st) < 0) { close(fd); return 0; }
  struct sigaction sa; memset(&sa, 0, sizeof sa); sa.sa_flags = SA_RESTART;
  sa.sa_handler = vt_on_rel; sigaction(SIGUSR1, &sa, NULL);
  sa.sa_handler = vt_on_acq; sigaction(SIGUSR2, &sa, NULL);
  struct vt_mode m = {.mode = VT_PROCESS, .relsig = SIGUSR1, .acqsig = SIGUSR2};
  if (ioctl(fd, VT_SETMODE, &m) < 0) { close(fd); return 0; }
  ioctl(fd, KDGKBMODE, &vt_kb); ioctl(fd, KDSKBMODE, K_OFF); ioctl(fd, KDSETMODE, KD_GRAPHICS);
  vt_fd = fd; vt_num = (int)minor(dev); vt_active = st.v_active == vt_num;
  return vt_num;
}
/* What the kernel asked since the last call, done: 0 nothing, 1 let go (the screen's master and the
   devices handed back, another VT shows), 2 taken back (the screen is this process's again and
   shows what it last showed; draw everything). */
int64_t cxvt_poll(void) {
  if (vt_fd < 0) return 0;
  if (vt_rel) {
    vt_rel = 0;
    if (scr_fd >= 0) drmDropMaster(scr_fd);
    for (int i = 0; i < in_n; i++) if (!in_file) ioctl(in_fd[i], EVIOCGRAB, 0);
    vt_active = 0;
    ioctl(vt_fd, VT_RELDISP, 1);
    return 1;
  }
  if (vt_acq) {
    vt_acq = 0;
    ioctl(vt_fd, VT_RELDISP, VT_ACKACQ);
    ioctl(vt_fd, KDSETMODE, KD_GRAPHICS);
    if (scr_fd >= 0) {
      drmSetMaster(scr_fd);
      struct pollfd q = {scr_fd, POLLIN, 0};
      if (poll(&q, 1, 0) > 0) { drmEventContext ev = {.version = 2, .page_flip_handler = scr_on_flip}; drmHandleEvent(scr_fd, &ev); }
      scr_flip_pending = 0;
      drmModeSetCrtc(scr_fd, scr_crtc, scr_buf[scr_back ^ 1].fb, 0, 0, &scr_conn, 1, &scr_mode);
      drmModeSetCursor(scr_fd, scr_crtc, 0, 0, 0);
    }
    struct input_event junk[64];
    for (int i = 0; i < in_n; i++) { if (in_file) continue; while (read(in_fd[i], junk, sizeof junk) > 0) {} ioctl(in_fd[i], EVIOCGRAB, 1); }
    vt_active = 1;
    return 2;
  }
  return 0;
}
/* Shows VT n (Ctrl+Alt+Fn): 0, or a negative errno. The kernel then asks this process to let go. */
int64_t cxvt_switch(int64_t n) {
  if (vt_fd < 0) return -ENOTTY;
  return ioctl(vt_fd, VT_ACTIVATE, (int)n) < 0 ? -(int64_t)errno : 0;
}
/* Hands the VT back to the console (after cxscr_close, so the screen is no longer held), and with
   to > 0 shows VT `to`, the session the person came from. */
int64_t cxvt_close(int64_t to) {
  if (vt_fd < 0) return 0;
  struct vt_mode m = {.mode = VT_AUTO};
  ioctl(vt_fd, VT_SETMODE, &m); ioctl(vt_fd, KDSETMODE, KD_TEXT);
  if (vt_kb >= 0) ioctl(vt_fd, KDSKBMODE, vt_kb);
  if (to > 0 && to != vt_num) ioctl(vt_fd, VT_ACTIVATE, (int)to);
  close(vt_fd); vt_fd = -1; vt_active = 1;
  return 0;
}
#else
int64_t cxscr_open(void) { return -1; }
int64_t cxscr_width(void) { return 0; }
int64_t cxscr_height(void) { return 0; }
int64_t cxscr_refresh(void) { return 0; }
int64_t cxscr_present(coax_v_u8 rgb, int64_t y0, int64_t y1) { (void)rgb; (void)y0; (void)y1; return -2; }
int64_t cxscr_close(void) { return 0; }
int64_t cxin_open(coax_v_u8 path) { (void)path; return -ENOSYS; }
int64_t cxin_read(void) { return -1; }
int64_t cxin_type(int64_t i) { (void)i; return -1; }
int64_t cxin_code(int64_t i) { (void)i; return -1; }
int64_t cxin_value(int64_t i) { (void)i; return 0; }
int64_t cxin_close(void) { return 0; }
int64_t cxvt_open(void) { return 0; }
int64_t cxvt_poll(void) { return 0; }
int64_t cxvt_switch(int64_t n) { (void)n; return -ENOSYS; }
int64_t cxvt_close(int64_t to) { (void)to; return 0; }
#endif
