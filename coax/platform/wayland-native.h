#ifndef COAX_WAYLAND_NATIVE_H
#define COAX_WAYLAND_NATIVE_H
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
/* The Wayland server's effects (os/compositor.coax). Effects only.
   cxwl_listen(path) / cxwl_unlisten(path): the server socket. cxwl_accept: a client handle, -1 for
   none. cxwl_recv(h, out): bytes ready (0 none, -1 gone), with the descriptors that came along in
   cxwl_fd_count / cxwl_fd(i). cxwl_send(h, bytes, fd or -1): 0 or -1. cxwl_hangup(h).
   cxwl_poll(ms, watch_stdin). Pools: cxwl_map(fd, size) a read-only map handle; cxwl_remap;
   cxwl_read(map, fd, offset, out) copies out of it; cxwl_unmap. cxwl_memfd(bytes): a sealed file.
   cxwl_close_fd, cxwl_ms (monotonic), cxwl_stdin(out) for the test channel. SIGPIPE is ignored. */
int64_t cxwl_listen(coax_v_u8 path);
int64_t cxwl_unlisten(coax_v_u8 path);
int64_t cxwl_accept(void);
/* The desk socket: listen on a path, accept a client (-1 none), its bytes (0 none, -1 gone), an answer, close. */
int64_t cxwl_desk_listen(coax_v_u8 path);
int64_t cxwl_desk_accept(void);
int64_t cxwl_desk_recv(int64_t h, coax_v_u8 out);
int64_t cxwl_desk_send(int64_t h, coax_v_u8 s);
int64_t cxwl_desk_close(int64_t h);
int64_t cxwl_recv(int64_t h, coax_v_u8 out);
int64_t cxwl_fd_count(void);
int64_t cxwl_fd(int64_t i);
int64_t cxwl_send(int64_t h, coax_v_u8 s, int64_t fd);
int64_t cxwl_hangup(int64_t h);
int64_t cxwl_close_fd(int64_t fd);
int64_t cxwl_poll(int64_t timeout_ms, int64_t watch_stdin);
int64_t cxwl_map(int64_t fd, int64_t size);
int64_t cxwl_unmap(int64_t m);
int64_t cxwl_remap(int64_t m, int64_t fd, int64_t size);
int64_t cxwl_read(int64_t m, int64_t fd, int64_t offset, coax_v_u8 out);
int64_t cxwl_memfd(coax_v_u8 s);
int64_t cxwl_ms(void);
int64_t cxwl_stdin(coax_v_u8 out);
int64_t cxwl_out(coax_v_u8 s);
int64_t cxwl_spawn(coax_v_u8 path, coax_v_u8 display);
int64_t cxwl_fits(int64_t m, int64_t fd, int64_t end);
int64_t cxwl_peer_pid(int64_t h);
int64_t cxwl_tree(int64_t pid);
int64_t cxwl_tree_at(int64_t i);
int64_t cxwl_ask_start(coax_v_u8 args);
int64_t cxwl_ask_poll(coax_v_u8 out);
int64_t cxwl_ask_len(void);
int64_t cxwl_run(coax_v_u8 args, coax_v_u8 display);
int64_t cxwl_xwayland(void);
int64_t cxwl_xwayland_handle(void);
int64_t cxx_send(coax_v_u8 b);
int64_t cxx_recv(coax_v_u8 out);
int64_t cxwl_xwayland_stop(void);
#ifdef __cplusplus
}
#endif
#endif
