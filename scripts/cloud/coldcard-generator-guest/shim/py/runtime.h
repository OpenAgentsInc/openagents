/*
 * MicroPython interface shim for compiling the pinned libngu `ngu/random.c`
 * outside a MicroPython firmware build (openagents #9297, OFR-015).
 *
 * The point of this file is to be as small as possible. `ngu/random.c` is
 * compiled VERBATIM from the pinned tree in the admitted guest image; nothing
 * in it is transcribed, edited, or reimplemented. It needs a MicroPython
 * runtime to compile, so this header supplies exactly the declarations it
 * mentions and nothing else.
 *
 * Nothing here participates in the generator arithmetic. Every declaration is
 * either a type used only in the Python-object wrappers we never call, or a
 * trap that aborts the harness. If a shim function is ever reached, the run
 * fails rather than producing a value this file invented.
 */

#ifndef OA_COLDCARD_MICROPYTHON_SHIM_RUNTIME_H
#define OA_COLDCARD_MICROPYTHON_SHIM_RUNTIME_H

#include <stddef.h>
#include <stdint.h>

/* MicroPython marks internal linkage with STATIC. */
#define STATIC static

#ifndef MIN
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#endif

/* Opaque Python object handle. random.c only passes these through. */
typedef const void *mp_obj_t;
typedef struct oa_mp_obj_dict mp_obj_dict_t;

typedef struct oa_mp_obj_base {
  const void *type;
} oa_mp_obj_base_t;

typedef struct oa_mp_obj_module {
  oa_mp_obj_base_t base;
  mp_obj_dict_t *globals;
} mp_obj_module_t;

typedef struct oa_mp_rom_map_elem {
  const void *key;
  const void *value;
} mp_rom_map_elem_t;

typedef struct oa_vstr {
  size_t alloc;
  size_t len;
  char *buf;
} vstr_t;

extern const int mp_type_module;
extern const int mp_type_bytes;
extern const mp_obj_t mp_const_none;

/* The qstr / const-object machinery is compile-time decoration in random.c:
 * the module table it builds is never consulted by this harness. Each macro
 * ignores its arguments, so no MP_QSTR_* identifier has to exist. */
#define MP_ROM_QSTR(q) ((const void *)0)
#define MP_ROM_PTR(p) ((const void *)(p))
#define MP_DEFINE_CONST_FUN_OBJ_0(name, fn) const void *const name = (const void *)0
#define MP_DEFINE_CONST_FUN_OBJ_1(name, fn) const void *const name = (const void *)0
#define MP_DEFINE_CONST_DICT(name, table) const void *const name = (const void *)0
#define MP_ERROR_TEXT(text) (text)

/* Traps. Reaching any of these aborts the run; see oa_shim.c. */
void mp_raise_OSError(int code) __attribute__((noreturn));
void mp_raise_ValueError(const char *message) __attribute__((noreturn));
mp_obj_t mp_obj_new_int_from_uint(uint32_t value);
mp_obj_t mp_obj_new_str_from_vstr(const void *type, vstr_t *vstr);
int mp_obj_get_int_truncated(mp_obj_t value);
void vstr_init_len(vstr_t *vstr, size_t len);

#endif /* OA_COLDCARD_MICROPYTHON_SHIM_RUNTIME_H */
