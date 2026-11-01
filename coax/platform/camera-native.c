/* The camera daemon's effects for Coax (os/camera-serve.coax): V4L2 capture of one camera, one
   v4l2loopback output node, and Unix-socket listeners with their connections for one poll loop.
   Effects only; every decision (formats to ask for, retries, the protocol, what to drop) is Coax.
   See camera-native.h. On a platform with no V4L2 the camera and loopback calls answer -ENOSYS,
   and the sockets still work. */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "camera-native.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>
#ifndef MSG_NOSIGNAL
#define MSG_NOSIGNAL 0
#endif
#if defined(__linux__)
#include <linux/videodev2.h>
#include <sys/ioctl.h>
#endif

static char *cstr(coax_v_u8 s) {
  char *p = malloc((size_t)s.count + 1);
  if (!p) return NULL;
  memcpy(p, s.data, (size_t)s.count); p[s.count] = 0;
  return p;
}

int64_t cxcam_strerror(int64_t err, coax_v_u8 out) {
  const char *t = strerror((int)err);
  size_t n = strlen(t);
  if ((int64_t)n > out.count) n = (size_t)out.count;
  memcpy(out.data, t, n);
  return (int64_t)n;
}

/* ---- the camera */

#define CAM_BUFS 4
static int cam_fd = -1;
#if defined(__linux__)
static void *cam_map[CAM_BUFS];
static size_t cam_len[CAM_BUFS];
#endif
static int cam_nbufs;
static int64_t cam_w, cam_h, cam_fourcc, cam_parm_err;

#if defined(__linux__)
static int xioctl(int fd, unsigned long req, void *arg) {
  int r;
  do { r = ioctl(fd, req, arg); } while (r < 0 && errno == EINTR);
  return r;
}
#endif

int64_t cxcam_close(void) {
#if defined(__linux__)
  if (cam_fd >= 0) {
    enum v4l2_buf_type t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    xioctl(cam_fd, VIDIOC_STREAMOFF, &t);
    for (int i = 0; i < cam_nbufs; i++) if (cam_map[i] && cam_map[i] != MAP_FAILED) munmap(cam_map[i], cam_len[i]);
    close(cam_fd);
  }
#endif
  cam_fd = -1; cam_nbufs = 0;
  return 0;
}

/* Opens the camera at w by h, MJPEG first and YUYV when the driver gives anything else, asks
   for fps, maps CAM_BUFS buffers and starts the stream. 0, or -errno with the stage in *stage
   (0 open, 1 format, 2 stream). cxcam_parm_error() is the errno of a refused frame rate (0
   when it was taken). */
int64_t cxcam_open(coax_v_u8 path, int64_t w, int64_t h, int64_t fps) {
  cxcam_close();
#if defined(__linux__)
  char *p = cstr(path);
  if (!p) return -ENOMEM;
  cam_fd = open(p, O_RDWR | O_NONBLOCK | O_CLOEXEC);
  free(p);
  if (cam_fd < 0) { cam_fd = -1; return -(int64_t)errno * 4 - 0; }
  struct v4l2_format f; memset(&f, 0, sizeof f);
  f.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
  if (xioctl(cam_fd, VIDIOC_G_FMT, &f) < 0) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 1; }
  f.fmt.pix.width = (uint32_t)w; f.fmt.pix.height = (uint32_t)h; f.fmt.pix.pixelformat = V4L2_PIX_FMT_MJPEG;
  int ok = xioctl(cam_fd, VIDIOC_S_FMT, &f) == 0 && f.fmt.pix.pixelformat == V4L2_PIX_FMT_MJPEG;
  if (!ok) {
    memset(&f, 0, sizeof f); f.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    if (xioctl(cam_fd, VIDIOC_G_FMT, &f) < 0) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 1; }
    f.fmt.pix.width = (uint32_t)w; f.fmt.pix.height = (uint32_t)h; f.fmt.pix.pixelformat = V4L2_PIX_FMT_YUYV;
    if (xioctl(cam_fd, VIDIOC_S_FMT, &f) < 0) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 1; }
  }
  cam_w = f.fmt.pix.width; cam_h = f.fmt.pix.height; cam_fourcc = f.fmt.pix.pixelformat;
  struct v4l2_streamparm sp; memset(&sp, 0, sizeof sp);
  sp.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
  sp.parm.capture.timeperframe.numerator = 1; sp.parm.capture.timeperframe.denominator = (uint32_t)fps;
  cam_parm_err = xioctl(cam_fd, VIDIOC_S_PARM, &sp) < 0 ? errno : 0;
  struct v4l2_requestbuffers rb; memset(&rb, 0, sizeof rb);
  rb.count = CAM_BUFS; rb.type = V4L2_BUF_TYPE_VIDEO_CAPTURE; rb.memory = V4L2_MEMORY_MMAP;
  if (xioctl(cam_fd, VIDIOC_REQBUFS, &rb) < 0) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 2; }
  cam_nbufs = rb.count > CAM_BUFS ? CAM_BUFS : (int)rb.count;
  for (int i = 0; i < cam_nbufs; i++) {
    struct v4l2_buffer b; memset(&b, 0, sizeof b);
    b.type = V4L2_BUF_TYPE_VIDEO_CAPTURE; b.memory = V4L2_MEMORY_MMAP; b.index = (uint32_t)i;
    if (xioctl(cam_fd, VIDIOC_QUERYBUF, &b) < 0) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 2; }
    cam_len[i] = b.length;
    cam_map[i] = mmap(NULL, b.length, PROT_READ | PROT_WRITE, MAP_SHARED, cam_fd, b.m.offset);
    if (cam_map[i] == MAP_FAILED) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 2; }
    if (xioctl(cam_fd, VIDIOC_QBUF, &b) < 0) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 2; }
  }
  enum v4l2_buf_type t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
  if (xioctl(cam_fd, VIDIOC_STREAMON, &t) < 0) { int e = errno; cxcam_close(); return -(int64_t)e * 4 - 2; }
  return 0;
#else
  (void)path; (void)w; (void)h; (void)fps;
  return -(int64_t)ENOSYS * 4;
#endif
}
int64_t cxcam_width(void) { return cam_w; }
int64_t cxcam_height(void) { return cam_h; }
int64_t cxcam_fourcc(void) { return cam_fourcc; }
int64_t cxcam_parm_error(void) { return cam_parm_err; }

/* The next frame's bytes into out: the count, 0 when none is ready, -errno on a failure. */
int64_t cxcam_frame(coax_v_u8 out) {
#if defined(__linux__)
  if (cam_fd < 0) return -EBADF;
  struct v4l2_buffer b; memset(&b, 0, sizeof b);
  b.type = V4L2_BUF_TYPE_VIDEO_CAPTURE; b.memory = V4L2_MEMORY_MMAP;
  if (xioctl(cam_fd, VIDIOC_DQBUF, &b) < 0) return errno == EAGAIN ? 0 : -(int64_t)errno;
  size_t n = b.bytesused ? b.bytesused : cam_len[b.index];
  if (n > cam_len[b.index]) n = cam_len[b.index];
  if ((int64_t)n > out.count) n = (size_t)out.count;
  memcpy(out.data, cam_map[b.index], n);
  if (xioctl(cam_fd, VIDIOC_QBUF, &b) < 0) return -(int64_t)errno;
  return (int64_t)n;
#else
  (void)out; return -ENOSYS;
#endif
}

/* ---- the loopback */

static int out_fd = -1;
static int64_t out_w, out_h, out_fourcc;

int64_t cxcam_out_close(void) { if (out_fd >= 0) close(out_fd); out_fd = -1; return 0; }
/* Opens a v4l2loopback node and sets its output format to YUYV w by h. 0; -errno*4 for the open
   (stage 0) or -errno*4-1 for the format (stage 1); 1 when the node took another format
   (cxcam_out_fourcc/width/height say which). */
int64_t cxcam_out_open(coax_v_u8 path, int64_t w, int64_t h) {
  cxcam_out_close();
#if defined(__linux__)
  char *p = cstr(path);
  if (!p) return -ENOMEM * 4;
  out_fd = open(p, O_RDWR | O_CLOEXEC);
  free(p);
  if (out_fd < 0) { out_fd = -1; return -(int64_t)errno * 4; }
  struct v4l2_format f; memset(&f, 0, sizeof f);
  f.type = V4L2_BUF_TYPE_VIDEO_OUTPUT;
  if (xioctl(out_fd, VIDIOC_G_FMT, &f) < 0) { int e = errno; cxcam_out_close(); return -(int64_t)e * 4 - 1; }
  f.fmt.pix.width = (uint32_t)w; f.fmt.pix.height = (uint32_t)h; f.fmt.pix.pixelformat = V4L2_PIX_FMT_YUYV;
  f.fmt.pix.field = V4L2_FIELD_NONE; f.fmt.pix.bytesperline = (uint32_t)(w * 2); f.fmt.pix.sizeimage = (uint32_t)(w * h * 2);
  if (xioctl(out_fd, VIDIOC_S_FMT, &f) < 0) { int e = errno; cxcam_out_close(); return -(int64_t)e * 4 - 1; }
  out_w = f.fmt.pix.width; out_h = f.fmt.pix.height; out_fourcc = f.fmt.pix.pixelformat;
  if (out_fourcc != V4L2_PIX_FMT_YUYV || out_w != w || out_h != h) { cxcam_out_close(); return 1; }
  return 0;
#else
  (void)path; (void)w; (void)h;
  return -(int64_t)ENOSYS * 4;
#endif
}
int64_t cxcam_out_width(void) { return out_w; }
int64_t cxcam_out_height(void) { return out_h; }
int64_t cxcam_out_fourcc(void) { return out_fourcc; }
/* One picture onto the node: 0, or -errno. */
int64_t cxcam_out_write(coax_v_u8 s) {
  if (out_fd < 0) return -EBADF;
  int64_t done = 0;
  while (done < s.count) {
    ssize_t n = write(out_fd, s.data + done, (size_t)(s.count - done));
    if (n < 0 && errno == EINTR) continue;
    if (n < 0) return -(int64_t)errno;
    if (n == 0) return -EIO;
    done += n;
  }
  return 0;
}

/* ---- Unix sockets */

#define SOCK_MAX 64
static int lis_fd[2] = {-1, -1};
static int con_fd[SOCK_MAX];
static int con_used[SOCK_MAX];
static int con_muted[SOCK_MAX];   /* written to, never read: left out of the poll */

/* Listens on path as listener `which` (0 or 1), removing what a dead daemon left there. 0 or -errno. */
int64_t cxcam_listen(int64_t which, coax_v_u8 path) {
  if (which < 0 || which > 1) return -EINVAL;
  signal(SIGPIPE, SIG_IGN);
  struct sockaddr_un a; memset(&a, 0, sizeof a); a.sun_family = AF_UNIX;
  if (path.count <= 0 || (size_t)path.count >= sizeof a.sun_path) return -ENAMETOOLONG;
  memcpy(a.sun_path, path.data, (size_t)path.count);
  unlink(a.sun_path);
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd < 0) return -(int64_t)errno;
  fcntl(fd, F_SETFD, FD_CLOEXEC);
  if (bind(fd, (struct sockaddr *)&a, sizeof a) < 0 || listen(fd, 16) < 0) { int e = errno; close(fd); return -(int64_t)e; }
  fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK);
  lis_fd[which] = fd;
  return 0;
}
int64_t cxcam_unlisten(int64_t which, coax_v_u8 path) {
  if (which < 0 || which > 1) return -EINVAL;
  if (lis_fd[which] >= 0) close(lis_fd[which]);
  lis_fd[which] = -1;
  char *p = cstr(path); if (p) { unlink(p); free(p); }
  return 0;
}
/* A new connection on a listener, non-blocking: its handle, or -1 when none waits (-2 when full). */
int64_t cxcam_accept(int64_t which) {
  if (which < 0 || which > 1 || lis_fd[which] < 0) return -1;
  int fd = accept(lis_fd[which], NULL, NULL);
  if (fd < 0) return -1;
  for (int i = 0; i < SOCK_MAX; i++) if (!con_used[i]) {
    fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK);
    fcntl(fd, F_SETFD, FD_CLOEXEC);
    con_used[i] = 1; con_muted[i] = 0; con_fd[i] = fd; return i;
  }
  close(fd);
  return -2;
}
/* What a connection has ready: the count read into out, 0 when nothing is, -1 at its end or on an error. */
int64_t cxcam_read(int64_t h, coax_v_u8 out) {
  if (h < 0 || h >= SOCK_MAX || !con_used[h]) return -1;
  ssize_t n = read(con_fd[h], out.data, (size_t)out.count);
  if (n < 0) return (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR) ? 0 : -1;
  return n == 0 ? -1 : (int64_t)n;
}
/* As much of s as the connection takes now: the count (0 when it is full), -1 on an error. */
int64_t cxcam_write(int64_t h, coax_v_u8 s) {
  if (h < 0 || h >= SOCK_MAX || !con_used[h]) return -1;
  ssize_t n = send(con_fd[h], s.data, (size_t)s.count, MSG_NOSIGNAL);
  if (n < 0) return (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR) ? 0 : -1;
  return (int64_t)n;
}
int64_t cxcam_mute(int64_t h) { if (h < 0 || h >= SOCK_MAX || !con_used[h]) return -1; con_muted[h] = 1; return 0; }
int64_t cxcam_hangup(int64_t h) {
  if (h < 0 || h >= SOCK_MAX || !con_used[h]) return -1;
  close(con_fd[h]); con_used[h] = 0; con_fd[h] = -1;
  return 0;
}

/* Waits up to timeout_ms until the camera has a frame, a listener a connection, or a connection
   bytes: the count of ready descriptors, 0 on timeout. */
int64_t cxcam_poll(int64_t timeout_ms) {
  struct pollfd p[2 + 1 + SOCK_MAX]; int n = 0;
  if (cam_fd >= 0) { p[n].fd = cam_fd; p[n].events = POLLIN; p[n].revents = 0; n++; }
  for (int i = 0; i < 2; i++) if (lis_fd[i] >= 0) { p[n].fd = lis_fd[i]; p[n].events = POLLIN; p[n].revents = 0; n++; }
  for (int i = 0; i < SOCK_MAX; i++) if (con_used[i] && !con_muted[i]) { p[n].fd = con_fd[i]; p[n].events = POLLIN; p[n].revents = 0; n++; }
  int r = poll(p, (nfds_t)n, (int)timeout_ms);
  return r < 0 ? 0 : r;
}
