#ifndef COAX_SCREEN_NATIVE_H
#define COAX_SCREEN_NATIVE_H
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
/* The compositor's screen and input (os/compositor.coax). Effects only.
   cxscr_open: the first connected screen at its preferred mode (0, or a negative failing step);
   cxscr_width/height/refresh; cxscr_present(rgb, offset): shows w*h*3 bytes at the next vertical
   blank (-1 while the last is still waiting); cxscr_close puts back what was shown before.
   cxin_open(path): keyboards and mice under /dev/input, grabbed, or with path a file of
   input_event records; cxin_read fills a list read by cxin_type/code/value(i); cxin_close. */
int64_t cxscr_open(void);
int64_t cxscr_width(void);
int64_t cxscr_height(void);
int64_t cxscr_refresh(void);
int64_t cxscr_present(coax_v_u8 rgb, int64_t offset);
int64_t cxscr_close(void);
int64_t cxin_open(coax_v_u8 path);
int64_t cxin_read(void);
int64_t cxin_type(int64_t i);
int64_t cxin_code(int64_t i);
int64_t cxin_value(int64_t i);
int64_t cxin_close(void);
#ifdef __cplusplus
}
#endif
#endif
