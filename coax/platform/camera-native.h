#ifndef COAX_CAMERA_NATIVE_H
#define COAX_CAMERA_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#ifdef COAX_coax_v_u8_DEFINED
#ifndef COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
#error "incompatible Coax C type coax_v_u8"
#endif
#else
#define COAX_coax_v_u8_DEFINED
#define COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
#endif
/* The camera daemon's effects (os/camera-serve.coax). A failure that has a stage answers
   -(errno * 4 + stage); one without answers -errno. cxcam_strerror writes errno's words.

   The camera: cxcam_open(path, w, h, fps) opens it, MJPEG first and YUYV when the driver gives
   anything else (stages 0 open, 1 format, 2 stream); cxcam_width/height/fourcc say what it took
   and cxcam_parm_error the errno of a refused frame rate (0 when taken). cxcam_frame(out) copies
   the next frame: its size, 0 when none is ready, -errno. cxcam_close.

   The loopback: cxcam_out_open(path, w, h) opens a v4l2loopback node with YUYV w by h (stages 0
   open, 1 format), 1 when the node took another format (cxcam_out_fourcc/width/height);
   cxcam_out_write(picture) is 0 or -errno; cxcam_out_close.

   Sockets: cxcam_listen(which, path) for listener 0 or 1; cxcam_accept(which) a connection
   handle, -1 for none; cxcam_read and cxcam_write never block (0 for nothing done, -1 at the end
   or on an error); cxcam_hangup; cxcam_unlisten(which, path) closes it and removes the file.
   cxcam_poll(ms) waits for the camera, a listener or a connection. SIGPIPE is ignored. */
int64_t cxcam_strerror(int64_t err, coax_v_u8 out);
int64_t cxcam_open(coax_v_u8 path, int64_t w, int64_t h, int64_t fps);
int64_t cxcam_width(void);
int64_t cxcam_height(void);
int64_t cxcam_fourcc(void);
int64_t cxcam_parm_error(void);
int64_t cxcam_frame(coax_v_u8 out);
int64_t cxcam_close(void);
int64_t cxcam_out_open(coax_v_u8 path, int64_t w, int64_t h);
int64_t cxcam_out_width(void);
int64_t cxcam_out_height(void);
int64_t cxcam_out_fourcc(void);
int64_t cxcam_out_write(coax_v_u8 s);
int64_t cxcam_out_close(void);
int64_t cxcam_listen(int64_t which, coax_v_u8 path);
int64_t cxcam_unlisten(int64_t which, coax_v_u8 path);
int64_t cxcam_accept(int64_t which);
int64_t cxcam_read(int64_t h, coax_v_u8 out);
int64_t cxcam_write(int64_t h, coax_v_u8 s);
int64_t cxcam_mute(int64_t h);   /* a connection only written to: not polled */
int64_t cxcam_hangup(int64_t h);
int64_t cxcam_poll(int64_t timeout_ms);
#ifdef __cplusplus
}
#endif
#endif
