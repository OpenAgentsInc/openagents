// verse-kms: a Verse realm on the display hardware, with no compositor and
// no browser. Linux DRM/KMS dumb buffers for the screen, evdev for the
// keyboard, and a CPU rasterizer spread over every core that calls the
// realm's light (its -light.coax) for each pixel. The realm's Coax steps
// the game and writes the strokes; this file is the driver: it decides no
// game rule and no colour.
//
//   verse-kms [--seconds N] [--scale K] [--shot FILE.ppm]     on the screen (root, a free VT)
//   verse-kms --headless WxH [--frames N] [--shot FILE.ppm]   no screen: render and save a frame
//
// On the screen it takes DRM master, so the compositor must have let go of
// it: run.sh switches to a free VT first. Escape (the realm leaves), the
// --seconds limit, or SIGINT/SIGTERM end it and restore the screen.
#define _GNU_SOURCE
#include "realm.h"
#include <errno.h>
#include <math.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#ifdef __linux__
#include <dirent.h>
#include <fcntl.h>
#include <linux/input.h>
#include <poll.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <xf86drm.h>
#include <xf86drmMode.h>
#endif

#define MAX_THREADS 64
#define BAND 16  // rows a worker takes at a time

static volatile sig_atomic_t stopping;
static void on_signal(int sig) { (void)sig; stopping = 1; }

static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
}

// ---- the rasterizer

typedef struct {
  int w, h, stride_px;  // stride in pixels of the target
  uint32_t *target;     // XRGB8888
  const float *v; int count, vf;
  const float *u;
  int next_band;        // atomic
} Frame;

static Frame frame;

static inline uint32_t encode(float r, float g, float b) {
  int ri = (int)(fminf(fmaxf(r, 0.f), 1.f) * 255.f + 0.5f);
  int gi = (int)(fminf(fmaxf(g, 0.f), 1.f) * 255.f + 0.5f);
  int bi = (int)(fminf(fmaxf(b, 0.f), 1.f) * 255.f + 0.5f);
  return (uint32_t)(ri << 16 | gi << 8 | bi);
}

// One band of rows [y0, y1): the background, then every stroke triangle
// that touches the band, added, then encoded into the target.
static void band(int y0, int y1, float *acc) {
  const Frame *f = &frame;
  int w = f->w;
  for (int y = y0; y < y1; y++) {
    float *row = acc + (size_t)(y - y0) * w * 3;
    for (int x = 0; x < w; x++) realm_bg(f->u, x + 0.5f, y + 0.5f, row + x * 3);
  }
  for (int t = 0; t + 2 < f->count; t += 3) {
    const float *a = f->v + (size_t)t * f->vf, *b = a + f->vf, *c = b + f->vf;
    float minx = fminf(a[0], fminf(b[0], c[0])), maxx = fmaxf(a[0], fmaxf(b[0], c[0]));
    float miny = fminf(a[1], fminf(b[1], c[1])), maxy = fmaxf(a[1], fmaxf(b[1], c[1]));
    if (maxy < y0 || miny >= y1 || maxx < 0 || minx >= w) continue;
    float area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if (fabsf(area) < 1e-6f) continue;
    float inv = 1.f / area;
    int ya = (int)fmaxf(floorf(miny), (float)y0), yb = (int)fminf(ceilf(maxy), (float)(y1 - 1));
    int xa = (int)fmaxf(floorf(minx), 0.f), xb = (int)fminf(ceilf(maxx), (float)(w - 1));
    const float *shape = a + 4;  // length, core, light, glow (flat over a stroke)
    const float *tint = a + 8;
    for (int y = ya; y <= yb; y++) {
      float py = y + 0.5f;
      float *row = acc + (size_t)(y - y0) * w * 3;
      for (int x = xa; x <= xb; x++) {
        float px = x + 0.5f;
        float w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) * inv;
        float w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) * inv;
        float w2 = 1.f - w0 - w1;
        if (w0 < 0 || w1 < 0 || w2 < 0) continue;
        float sx = w0 * a[2] + w1 * b[2] + w2 * c[2];
        float sy = w0 * a[3] + w1 * b[3] + w2 * c[3];
        float k = realm_fx(f->u, sx, sy, shape);
        row[x * 3] += tint[0] * k; row[x * 3 + 1] += tint[1] * k; row[x * 3 + 2] += tint[2] * k;
      }
    }
  }
  for (int y = y0; y < y1; y++) {
    const float *row = acc + (size_t)(y - y0) * w * 3;
    uint32_t *out = f->target + (size_t)y * f->stride_px;
    for (int x = 0; x < w; x++) out[x] = encode(row[x * 3], row[x * 3 + 1], row[x * 3 + 2]);
  }
}

static void *worker(void *arg) {
  float *acc = arg;
  for (;;) {
    int y0 = __atomic_fetch_add(&frame.next_band, BAND, __ATOMIC_RELAXED);
    if (y0 >= frame.h) break;
    int y1 = y0 + BAND < frame.h ? y0 + BAND : frame.h;
    band(y0, y1, acc);
  }
  return NULL;
}

static int threads;
static float *accs[MAX_THREADS];

static void draw(uint32_t *target, int w, int h, int stride_px, const float *v, int count, const float *u) {
  frame = (Frame){w, h, stride_px, target, v, count, realm_vertex_floats(), u, 0};
  pthread_t t[MAX_THREADS];
  for (int i = 0; i < threads; i++) pthread_create(&t[i], NULL, worker, accs[i]);
  for (int i = 0; i < threads; i++) pthread_join(t[i], NULL);
}

static int save_ppm(const char *path, const uint32_t *px, int w, int h, int stride_px) {
  FILE *f = fopen(path, "wb");
  if (!f) return -1;
  fprintf(f, "P6\n%d %d\n255\n", w, h);
  for (int y = 0; y < h; y++)
    for (int x = 0; x < w; x++) {
      uint32_t p = px[(size_t)y * stride_px + x];
      unsigned char rgb[3] = {(unsigned char)(p >> 16), (unsigned char)(p >> 8), (unsigned char)p};
      fwrite(rgb, 1, 3, f);
    }
  return fclose(f);
}

// ---- the realm

static void *state;
static float *vertices, *uniforms;

static int step(int w, int h, float scale, float dt) {
  return realm_frame(state, vertices, uniforms, (float)w, (float)h, scale, dt);
}

// ---- the keyboard (evdev)

#ifdef __linux__
#define MAX_KEYBOARDS 16
static int keyboards[MAX_KEYBOARDS], keyboard_count;

static int key_of(int code) {
  switch (code) {
    case KEY_ESC: return REALM_ESCAPE;
    case KEY_ENTER: case KEY_KPENTER: return REALM_ENTER;
    case KEY_BACKSPACE: return REALM_BACKSPACE;
    case KEY_LEFT: return REALM_LEFT;
    case KEY_RIGHT: return REALM_RIGHT;
    case KEY_UP: return REALM_UP;
    case KEY_DOWN: return REALM_DOWN;
    case KEY_SPACE: return ' ';
  }
  static const struct { int code; char c; } letters[] = {
    {KEY_A,'a'},{KEY_B,'b'},{KEY_C,'c'},{KEY_D,'d'},{KEY_E,'e'},{KEY_F,'f'},{KEY_G,'g'},{KEY_H,'h'},{KEY_I,'i'},
    {KEY_J,'j'},{KEY_K,'k'},{KEY_L,'l'},{KEY_M,'m'},{KEY_N,'n'},{KEY_O,'o'},{KEY_P,'p'},{KEY_Q,'q'},{KEY_R,'r'},
    {KEY_S,'s'},{KEY_T,'t'},{KEY_U,'u'},{KEY_V,'v'},{KEY_W,'w'},{KEY_X,'x'},{KEY_Y,'y'},{KEY_Z,'z'},
    {KEY_1,'1'},{KEY_2,'2'},{KEY_3,'3'},{KEY_4,'4'},{KEY_5,'5'},{KEY_6,'6'},{KEY_7,'7'},{KEY_8,'8'},{KEY_9,'9'},{KEY_0,'0'}};
  for (size_t i = 0; i < sizeof letters / sizeof letters[0]; i++)
    if (letters[i].code == code) return letters[i].c;
  return 0;
}

// Every device that has a space bar is a keyboard. Grab it, so keys reach
// the realm and not the console under it.
static void open_keyboards(void) {
  DIR *d = opendir("/dev/input");
  if (!d) return;
  struct dirent *e;
  while ((e = readdir(d)) && keyboard_count < MAX_KEYBOARDS) {
    if (strncmp(e->d_name, "event", 5) != 0) continue;
    char path[300];
    snprintf(path, sizeof path, "/dev/input/%s", e->d_name);
    int fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
    if (fd < 0) continue;
    unsigned long bits[KEY_MAX / (8 * sizeof(unsigned long)) + 1] = {0};
    if (ioctl(fd, EVIOCGBIT(EV_KEY, sizeof bits), bits) < 0 ||
        !(bits[KEY_SPACE / (8 * sizeof(unsigned long))] & (1UL << (KEY_SPACE % (8 * sizeof(unsigned long)))))) {
      close(fd);
      continue;
    }
    ioctl(fd, EVIOCGRAB, 1);
    keyboards[keyboard_count++] = fd;
  }
  closedir(d);
}

static void read_keys(void) {
  struct input_event ev[64];
  for (int i = 0; i < keyboard_count; i++) {
    ssize_t n;
    while ((n = read(keyboards[i], ev, sizeof ev)) > 0) {
      for (size_t j = 0; j < (size_t)n / sizeof ev[0]; j++) {
        if (ev[j].type != EV_KEY || ev[j].value == 2) continue;
        int k = key_of(ev[j].code);
        if (!k) continue;
        if (ev[j].value == 1) realm_key(state, k);
        else realm_key_up(state, k);
      }
    }
  }
}

static void close_keyboards(void) {
  for (int i = 0; i < keyboard_count; i++) {
    ioctl(keyboards[i], EVIOCGRAB, 0);
    close(keyboards[i]);
  }
}

// ---- the screen (DRM/KMS dumb buffers)

typedef struct { uint32_t handle, pitch, fb; uint64_t size; uint32_t *map; } Buffer;

static int make_buffer(int fd, int w, int h, Buffer *b) {
  struct drm_mode_create_dumb create = {.width = (uint32_t)w, .height = (uint32_t)h, .bpp = 32};
  if (drmIoctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &create) < 0) return -1;
  b->handle = create.handle; b->pitch = create.pitch; b->size = create.size;
  if (drmModeAddFB(fd, (uint32_t)w, (uint32_t)h, 24, 32, b->pitch, b->handle, &b->fb) < 0) return -1;
  struct drm_mode_map_dumb map = {.handle = b->handle};
  if (drmIoctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &map) < 0) return -1;
  b->map = mmap(0, b->size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, (off_t)map.offset);
  return b->map == MAP_FAILED ? -1 : 0;
}

static void free_buffer(int fd, Buffer *b) {
  if (b->map && b->map != MAP_FAILED) munmap(b->map, b->size);
  if (b->fb) drmModeRmFB(fd, b->fb);
  struct drm_mode_destroy_dumb d = {.handle = b->handle};
  if (b->handle) drmIoctl(fd, DRM_IOCTL_MODE_DESTROY_DUMB, &d);
}

static int flip_pending;
static void on_flip(int fd, unsigned seq, unsigned s, unsigned us, void *data) {
  (void)fd; (void)seq; (void)s; (void)us; (void)data;
  flip_pending = 0;
}

static int run_screen(double seconds, float scale, const char *shot) {
  const char *card = getenv("VERSE_KMS_CARD");
  int fd = -1;
  if (card) fd = open(card, O_RDWR | O_CLOEXEC);
  for (int i = 0; fd < 0 && i < 8; i++) {
    char path[32];
    snprintf(path, sizeof path, "/dev/dri/card%d", i);
    fd = open(path, O_RDWR | O_CLOEXEC);
    if (fd >= 0) {
      drmModeRes *r = drmModeGetResources(fd);
      if (r && r->count_connectors > 0) { drmModeFreeResources(r); break; }
      if (r) drmModeFreeResources(r);
      close(fd); fd = -1;
    }
  }
  if (fd < 0) { fprintf(stderr, "verse-kms: no display device\n"); return 1; }
  // The paused session's logind lets go of master a moment after the VT
  // switch; wait for it rather than racing it.
  for (int i = 0; i < 50 && drmSetMaster(fd) < 0 && errno == EBUSY; i++) usleep(100000);
  if (drmSetMaster(fd) < 0 && !drmIsMaster(fd)) {
    fprintf(stderr, "verse-kms: another program holds the display (%s); switch to a free VT first (run.sh does)\n", strerror(errno));
    return 1;
  }
  drmModeRes *res = drmModeGetResources(fd);
  drmModeConnector *conn = NULL;
  for (int i = 0; res && i < res->count_connectors; i++) {
    drmModeConnector *c = drmModeGetConnector(fd, res->connectors[i]);
    if (c && c->connection == DRM_MODE_CONNECTED && c->count_modes > 0) { conn = c; break; }
    if (c) drmModeFreeConnector(c);
  }
  if (!conn) { fprintf(stderr, "verse-kms: no connected screen\n"); return 1; }
  drmModeModeInfo mode = conn->modes[0];
  for (int i = 0; i < conn->count_modes; i++)
    if (conn->modes[i].type & DRM_MODE_TYPE_PREFERRED) { mode = conn->modes[i]; break; }
  uint32_t crtc_id = 0;
  drmModeEncoder *enc = conn->encoder_id ? drmModeGetEncoder(fd, conn->encoder_id) : NULL;
  if (enc && enc->crtc_id) crtc_id = enc->crtc_id;
  for (int i = 0; !crtc_id && i < conn->count_encoders; i++) {
    drmModeEncoder *e = drmModeGetEncoder(fd, conn->encoders[i]);
    for (int j = 0; e && j < res->count_crtcs; j++)
      if (e->possible_crtcs & (1u << j)) { crtc_id = res->crtcs[j]; break; }
    if (e) drmModeFreeEncoder(e);
  }
  if (enc) drmModeFreeEncoder(enc);
  if (!crtc_id) { fprintf(stderr, "verse-kms: no CRTC for the screen\n"); return 1; }
  drmModeCrtc *saved = drmModeGetCrtc(fd, crtc_id);
  int w = mode.hdisplay, h = mode.vdisplay;
  Buffer buf[2] = {0};
  if (make_buffer(fd, w, h, &buf[0]) < 0 || make_buffer(fd, w, h, &buf[1]) < 0) {
    fprintf(stderr, "verse-kms: dumb buffers failed: %s\n", strerror(errno));
    return 1;
  }
  open_keyboards();
  fprintf(stderr, "verse-kms: %s on %dx%d@%d, %d threads, %d keyboards\n", realm_name(), w, h, mode.vrefresh, threads, keyboard_count);
  int back = 0, shown = 0, frames = 0, status = 0;
  double start = now_s(), last = start, busy = 0;
  drmEventContext ev = {.version = 2, .page_flip_handler = on_flip};
  while (!stopping && now_s() - start < seconds) {
    read_keys();
    double t = now_s();
    int n = step(w, h, scale, (float)(t - last));
    last = t;
    draw(buf[back].map, w, h, (int)(buf[back].pitch / 4), vertices, n, uniforms);
    busy += now_s() - t;
    if (realm_action(state)) break;
    if (!shown) {
      if (drmModeSetCrtc(fd, crtc_id, buf[back].fb, 0, 0, &conn->connector_id, 1, &mode) < 0) {
        fprintf(stderr, "verse-kms: mode set failed: %s\n", strerror(errno));
        status = 1;
        break;
      }
      shown = 1;
    } else {
      flip_pending = 1;
      if (drmModePageFlip(fd, crtc_id, buf[back].fb, DRM_MODE_PAGE_FLIP_EVENT, NULL) < 0) {
        flip_pending = 0;
        drmModeSetCrtc(fd, crtc_id, buf[back].fb, 0, 0, &conn->connector_id, 1, &mode);
      }
      while (flip_pending && !stopping) {
        struct pollfd p = {.fd = fd, .events = POLLIN};
        if (poll(&p, 1, 100) > 0) drmHandleEvent(fd, &ev);
      }
    }
    if (shot && frames == 30) save_ppm(shot, buf[back].map, w, h, (int)(buf[back].pitch / 4));
    back ^= 1;
    frames++;
  }
  double total = now_s() - start;
  fprintf(stderr, "verse-kms: %d frames in %.1f s (%.1f fps), %.2f ms drawing a frame\n", frames, total, frames / total, frames ? busy / frames * 1000 : 0);
  close_keyboards();
  if (saved) {
    drmModeSetCrtc(fd, saved->crtc_id, saved->buffer_id, saved->x, saved->y, &conn->connector_id, 1, &saved->mode);
    drmModeFreeCrtc(saved);
  }
  free_buffer(fd, &buf[0]);
  free_buffer(fd, &buf[1]);
  drmModeFreeConnector(conn);
  drmModeFreeResources(res);
  drmDropMaster(fd);
  close(fd);
  return status;
}
#endif

// ---- headless: the same frames, into memory

static int run_headless(int w, int h, int frames, float scale, const char *shot) {
  uint32_t *px = calloc((size_t)w * h, 4);
  if (!px) return 1;
  double busy = 0;
  for (int i = 0; i < frames; i++) {
    if (i == 20) realm_key(state, ' ');  // start the game so stars fall
    double t = now_s();
    int n = step(w, h, scale, 1.f / 60.f);
    draw(px, w, h, w, vertices, n, uniforms);
    busy += now_s() - t;
  }
  fprintf(stderr, "verse-kms: headless %s %dx%d, %d frames, %.2f ms a frame on %d threads\n", realm_name(), w, h, frames, busy / frames * 1000, threads);
  int status = shot ? save_ppm(shot, px, w, h, w) : 0;
  free(px);
  return status;
}

int main(int argc, char **argv) {
  double seconds = 60;
  float scale = 2;
  int hw = 0, hh = 0, frames = 120;
  const char *shot = NULL;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--seconds") && i + 1 < argc) seconds = atof(argv[++i]);
    else if (!strcmp(argv[i], "--scale") && i + 1 < argc) scale = (float)atof(argv[++i]);
    else if (!strcmp(argv[i], "--shot") && i + 1 < argc) shot = argv[++i];
    else if (!strcmp(argv[i], "--frames") && i + 1 < argc) frames = atoi(argv[++i]);
    else if (!strcmp(argv[i], "--headless") && i + 1 < argc) { if (sscanf(argv[++i], "%dx%d", &hw, &hh) != 2) hw = 0; }
    else { fprintf(stderr, "usage: verse-kms [--seconds N] [--scale K] [--shot F.ppm] | --headless WxH [--frames N] [--shot F.ppm]\n"); return 2; }
  }
  long cores = sysconf(_SC_NPROCESSORS_ONLN);
  threads = cores < 1 ? 1 : cores > MAX_THREADS ? MAX_THREADS : (int)cores;
  int maxw = hw ? hw : 7680;
  for (int i = 0; i < threads; i++) accs[i] = malloc((size_t)maxw * BAND * 3 * sizeof(float));
  state = calloc(1, realm_state_size());
  vertices = calloc((size_t)realm_max_vertices() * realm_vertex_floats(), sizeof(float));
  uniforms = calloc((size_t)realm_uniform_floats(), sizeof(float));
  if (!state || !vertices || !uniforms) return 1;
  signal(SIGINT, on_signal);
  signal(SIGTERM, on_signal);
  if (hw > 0 && hh > 0) return run_headless(hw, hh, frames, scale, shot);
#ifdef __linux__
  return run_screen(seconds, scale, shot);
#else
  fprintf(stderr, "verse-kms: the screen needs Linux; use --headless WxH\n");
  return 2;
#endif
}
