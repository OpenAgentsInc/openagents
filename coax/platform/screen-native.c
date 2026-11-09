/* The compositor's faces to the machine (os/compositor.coax): every monitor on every card through DRM/KMS dumb buffers,
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
/* One monitor: a connector on a card, the CRTC that drives it, its mode and two dumb buffers. */
typedef struct {
  int fd, w, h, back, flip_pending;
  uint32_t crtc, conn;
  drmModeModeInfo mode;
  drmModeCrtc *saved;
  ScrBuf buf[2];
  char name[32];
} ScrHead;
#define SCR_HEADS 8
#define SCR_CARDS 8
static ScrHead scr_head[SCR_HEADS];
static int scr_heads;
static int scr_card[SCR_CARDS], scr_cards;   /* the cards this process is master of, the main one first */
/* The VT the session runs on, when it has one: switches away and back are this process's to grant. */
static int vt_fd = -1, vt_num, vt_kb = -1, vt_active = 1;
static volatile sig_atomic_t vt_rel, vt_acq;

static int scr_make(int fd, int w, int h, ScrBuf *b) {
  struct drm_mode_create_dumb c = {.width = (uint32_t)w, .height = (uint32_t)h, .bpp = 32};
  if (drmIoctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &c) < 0) return -1;
  b->handle = c.handle; b->pitch = c.pitch; b->size = c.size;
  if (drmModeAddFB(fd, (uint32_t)w, (uint32_t)h, 24, 32, b->pitch, b->handle, &b->fb) < 0) return -1;
  struct drm_mode_map_dumb m = {.handle = b->handle};
  if (drmIoctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &m) < 0) return -1;
  b->map = mmap(0, b->size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, (off_t)m.offset);
  if (b->map == MAP_FAILED) { b->map = NULL; return -1; }
  memset(b->map, 0, b->size);
  return 0;
}
static void scr_free(int fd, ScrBuf *b) {
  if (b->map) munmap(b->map, b->size);
  if (b->fb) drmModeRmFB(fd, b->fb);
  struct drm_mode_destroy_dumb d = {.handle = b->handle};
  if (b->handle) drmIoctl(fd, DRM_IOCTL_MODE_DESTROY_DUMB, &d);
  memset(b, 0, sizeof *b);
}
static void scr_on_flip(int fd, unsigned seq, unsigned s, unsigned us, unsigned crtc, void *data) {
  (void)seq; (void)s; (void)us; (void)data;
  for (int i = 0; i < scr_heads; i++) if (scr_head[i].fd == fd && scr_head[i].crtc == crtc) scr_head[i].flip_pending = 0;
}
/* Reads every flip a card has finished. */
static void scr_drain(int fd) {
  struct pollfd p = {fd, POLLIN, 0};
  if (poll(&p, 1, 0) > 0) { drmEventContext ev = {.version = 3, .page_flip_handler2 = scr_on_flip}; drmHandleEvent(fd, &ev); }
}
/* Whether the firmware showed its console on this card: the main card, the one whose monitor shows
   desk 1, when no card is named. */
static int scr_boot_card(const char *path) {
  const char *slash = strrchr(path, '/');
  char sys[96]; snprintf(sys, sizeof sys, "/sys/class/drm/%s/device/boot_vga", slash ? slash + 1 : path);
  FILE *f = fopen(sys, "r"); if (!f) return 0;
  int c = fgetc(f); fclose(f);
  return c == '1';
}
/* Takes one card: master, then a head for each connected connector at its preferred mode, each on
   a CRTC of its own. The heads it added, or a negative step: -1 no device, -2 another program
   holds it. */
static int scr_take_card(const char *path) {
  int fd = open(path, O_RDWR | O_CLOEXEC);
  if (fd < 0) return -1;
  drmModeRes *probe = drmModeGetResources(fd);
  int any = probe && probe->count_connectors > 0; if (probe) drmModeFreeResources(probe);
  if (!any) { close(fd); return 0; }
  for (int i = 0; i < 50 && drmSetMaster(fd) < 0 && errno == EBUSY; i++) usleep(100000);
  if (drmSetMaster(fd) < 0 && !drmIsMaster(fd)) { close(fd); return -2; }
  drmModeRes *res = drmModeGetResources(fd);
  int added = 0;
  uint32_t used = 0;   /* the CRTCs (by index) this card's heads took */
  for (int i = 0; res && i < res->count_connectors && scr_heads < SCR_HEADS; i++) {
    drmModeConnector *conn = drmModeGetConnector(fd, res->connectors[i]);
    if (!conn || conn->connection != DRM_MODE_CONNECTED || conn->count_modes == 0) { if (conn) drmModeFreeConnector(conn); continue; }
    ScrHead *h = &scr_head[scr_heads]; memset(h, 0, sizeof *h);
    h->fd = fd; h->conn = conn->connector_id;
    h->mode = conn->modes[0];
    for (int m = 0; m < conn->count_modes; m++) if (conn->modes[m].type & DRM_MODE_TYPE_PREFERRED) { h->mode = conn->modes[m]; break; }
    const char *type = drmModeGetConnectorTypeName(conn->connector_type);
    snprintf(h->name, sizeof h->name, "%s-%u", type ? type : "Unknown", conn->connector_type_id);
    /* the CRTC the connector already shows on, when no other head took it; else the first free one it can reach */
    drmModeEncoder *enc = conn->encoder_id ? drmModeGetEncoder(fd, conn->encoder_id) : NULL;
    for (int j = 0; enc && enc->crtc_id && j < res->count_crtcs; j++)
      if (res->crtcs[j] == enc->crtc_id && !(used & (1u << j))) { h->crtc = enc->crtc_id; used |= 1u << j; }
    if (enc) drmModeFreeEncoder(enc);
    for (int e = 0; !h->crtc && e < conn->count_encoders; e++) {
      drmModeEncoder *x = drmModeGetEncoder(fd, conn->encoders[e]);
      for (int j = 0; x && j < res->count_crtcs; j++) if ((x->possible_crtcs & (1u << j)) && !(used & (1u << j))) { h->crtc = res->crtcs[j]; used |= 1u << j; break; }
      if (x) drmModeFreeEncoder(x);
    }
    drmModeFreeConnector(conn);
    if (!h->crtc) continue;   /* every CRTC it can reach drives another monitor: it stays dark */
    h->saved = drmModeGetCrtc(fd, h->crtc);
    h->w = h->mode.hdisplay; h->h = h->mode.vdisplay;
    if (scr_make(fd, h->w, h->h, &h->buf[0]) < 0 || scr_make(fd, h->w, h->h, &h->buf[1]) < 0 ||
        drmModeSetCrtc(fd, h->crtc, h->buf[0].fb, 0, 0, &h->conn, 1, &h->mode) < 0) {
      scr_free(fd, &h->buf[0]); scr_free(fd, &h->buf[1]);
      if (h->saved) { drmModeFreeCrtc(h->saved); h->saved = NULL; }
      continue;
    }
    drmModeSetCursor(fd, h->crtc, 0, 0, 0);   /* a cursor another session left on the CRTC: off (the pointer is drawn in the frame) */
    h->back = 1;
    scr_heads++; added++;
  }
  if (res) drmModeFreeResources(res);
  if (added == 0) { drmDropMaster(fd); close(fd); return 0; }
  scr_card[scr_cards++] = fd;
  return added;
}
/* Takes every monitor: the cards CODEROS_COMPOSITOR_CARD names (one path, or several split by ':',
   the main card first), or else every /dev/dri/card0..7, the card the firmware booted on first. A
   card another program holds is passed over. The count of monitors, or a negative step when there
   is none: -1 no device, -2 another program holds every card, -3 no monitor is connected. */
int64_t cxscr_open_all(void) {
  if (scr_heads > 0) return scr_heads;
  char paths[SCR_CARDS][64]; int n = 0;
  const char *named = getenv("CODEROS_COMPOSITOR_CARD");
  if (named && *named) {
    const char *p = named;
    while (*p && n < SCR_CARDS) {
      const char *e = strchr(p, ':'); size_t len = e ? (size_t)(e - p) : strlen(p);
      if (len > 0 && len < 64) { memcpy(paths[n], p, len); paths[n][len] = 0; n++; }
      if (!e) break;
      p = e + 1;
    }
  } else {
    for (int boot = 1; boot >= 0; boot--)
      for (int i = 0; i < SCR_CARDS && n < SCR_CARDS; i++) {
        char path[64]; snprintf(path, sizeof path, "/dev/dri/card%d", i);
        if (access(path, F_OK) != 0 || scr_boot_card(path) != boot) continue;
        snprintf(paths[n++], 64, "%s", path);
      }
  }
  int opened = 0, held = 0;
  for (int i = 0; i < n; i++) {
    int r = scr_take_card(paths[i]);
    if (r == -2) held++; else if (r >= 0) opened++;
  }
  if (scr_heads > 0) return scr_heads;
  return opened == 0 && held == 0 ? -1 : held > 0 ? -2 : -3;
}
int64_t cxscr_heads(void) { return scr_heads; }
int64_t cxscr_head_width(int64_t i) { return i >= 0 && i < scr_heads ? scr_head[i].w : 0; }
int64_t cxscr_head_height(int64_t i) { return i >= 0 && i < scr_heads ? scr_head[i].h : 0; }
int64_t cxscr_head_refresh(int64_t i) { return i >= 0 && i < scr_heads ? scr_head[i].mode.vrefresh : 0; }
/* The head's connector name, as the kernel names it (HDMI-A-2): its length, or -1. */
int64_t cxscr_head_name(int64_t i, coax_v_u8 out) {
  if (i < 0 || i >= scr_heads) return -1;
  size_t len = strlen(scr_head[i].name);
  if ((int64_t)len > out.count) return -1;
  memcpy(out.data, scr_head[i].name, len);
  return (int64_t)len;
}
/* Shows rgb (the head's w*h*3 bytes, its own rows) on head i at the next vertical blank, copying
   only rows y0..y1 into the back buffer (the rest of it already holds them): 0, -1 when the last
   one has not been shown yet (nothing changes), -2 on an error. */
int64_t cxscr_head_present(int64_t i, coax_v_u8 rgb, int64_t y0, int64_t y1) {
  if (i < 0 || i >= scr_heads) return -2;
  ScrHead *h = &scr_head[i];
  if (!vt_active) return -1;   /* another VT is showing: the frame waits for this one */
  if (h->flip_pending) { scr_drain(h->fd); if (h->flip_pending) return -1; }
  if (rgb.count < (int64_t)h->w * h->h * 3) return -2;
  if (y0 < 0) y0 = 0;
  if (y1 > h->h) y1 = h->h;
  ScrBuf *b = &h->buf[h->back];
  const uint8_t *s = rgb.data;
  uint32_t row[8192];
  int w = h->w < 8192 ? h->w : 8192;
  for (int y = (int)y0; y < (int)y1; y++) {   // whole rows: the buffer is write-combined
    const uint8_t *p = s + (size_t)y * h->w * 3;
    for (int x = 0; x < w; x++) row[x] = ((uint32_t)p[3 * x] << 16) | ((uint32_t)p[3 * x + 1] << 8) | p[3 * x + 2];
    memcpy((char *)b->map + (size_t)y * b->pitch, row, (size_t)w * 4);
  }
  if (drmModePageFlip(h->fd, h->crtc, b->fb, DRM_MODE_PAGE_FLIP_EVENT, NULL) < 0) return -2;
  h->flip_pending = 1; h->back ^= 1;
  return 0;
}
/* The first monitor alone, as before there were several: 0, or the negative step. */
int64_t cxscr_open(void) { int64_t r = cxscr_open_all(); return r > 0 ? 0 : r; }
int64_t cxscr_width(void) { return cxscr_head_width(0); }
int64_t cxscr_height(void) { return cxscr_head_height(0); }
int64_t cxscr_refresh(void) { return cxscr_head_refresh(0); }
int64_t cxscr_present(coax_v_u8 rgb, int64_t y0, int64_t y1) { return cxscr_head_present(0, rgb, y0, y1); }
/* Puts back what each monitor showed before and lets go of every card. */
int64_t cxscr_close(void) {
  for (int i = 0; i < scr_heads; i++) {
    ScrHead *h = &scr_head[i];
    if (h->saved) { drmModeSetCrtc(h->fd, h->saved->crtc_id, h->saved->buffer_id, h->saved->x, h->saved->y, &h->conn, 1, &h->saved->mode); drmModeFreeCrtc(h->saved); h->saved = NULL; }
    scr_free(h->fd, &h->buf[0]); scr_free(h->fd, &h->buf[1]);
  }
  scr_heads = 0;
  for (int c = 0; c < scr_cards; c++) { drmDropMaster(scr_card[c]); close(scr_card[c]); }
  scr_cards = 0;
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
    for (int c = 0; c < scr_cards; c++) drmDropMaster(scr_card[c]);
    for (int i = 0; i < in_n; i++) if (!in_file) ioctl(in_fd[i], EVIOCGRAB, 0);
    vt_active = 0;
    ioctl(vt_fd, VT_RELDISP, 1);
    return 1;
  }
  if (vt_acq) {
    vt_acq = 0;
    ioctl(vt_fd, VT_RELDISP, VT_ACKACQ);
    ioctl(vt_fd, KDSETMODE, KD_GRAPHICS);
    for (int c = 0; c < scr_cards; c++) { drmSetMaster(scr_card[c]); scr_drain(scr_card[c]); }
    for (int i = 0; i < scr_heads; i++) {
      ScrHead *h = &scr_head[i];
      h->flip_pending = 0;
      drmModeSetCrtc(h->fd, h->crtc, h->buf[h->back ^ 1].fb, 0, 0, &h->conn, 1, &h->mode);
      drmModeSetCursor(h->fd, h->crtc, 0, 0, 0);
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
int64_t cxscr_open_all(void) { return -1; }
int64_t cxscr_heads(void) { return 0; }
int64_t cxscr_head_width(int64_t i) { (void)i; return 0; }
int64_t cxscr_head_height(int64_t i) { (void)i; return 0; }
int64_t cxscr_head_refresh(int64_t i) { (void)i; return 0; }
int64_t cxscr_head_name(int64_t i, coax_v_u8 out) { (void)i; (void)out; return -1; }
int64_t cxscr_head_present(int64_t i, coax_v_u8 rgb, int64_t y0, int64_t y1) { (void)i; (void)rgb; (void)y0; (void)y1; return -2; }
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
