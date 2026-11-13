// The script module's C face for the browser: scalar Wasm exports over script-host.coax, for
// coax/web/host.js ("A realm's scripts", GitHub #13467). The host keeps one ScriptPage above the
// module's own data, copies bytes into `in` and reads answers from `out`.
#include <stddef.h>
#include <stdint.h>
#include "script-host.h"

#define EXPORT(name) __attribute__((export_name(name)))
static uint8_t in[131072];    /* a list (world.scripts.v1), a module, a frame, the realm's wants */
static uint8_t out[32768];    /* a result, a status, an address */
static coax_v_t_ScriptPage at(uint32_t state) { return (coax_v_t_ScriptPage){(coax_t_ScriptPage *)(uintptr_t)state, 1}; }

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_ScriptPage); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_ScriptPage((coax_t_ScriptPage *)(uintptr_t)state); }
EXPORT("scripts_in") uint32_t scripts_in(void) { return (uint32_t)(uintptr_t)in; }
EXPORT("scripts_in_cap") uint32_t scripts_in_cap(void) { return sizeof in; }
EXPORT("scripts_out") uint32_t scripts_out(void) { return (uint32_t)(uintptr_t)out; }
EXPORT("scripts_want") uint32_t scripts_want(uint32_t state, uint32_t n) {
  if (n > sizeof in) return 0;
  return coaxial_scripts_want(at(state), (coax_string){(const char *)in, (int64_t)n}) ? 1u : 0u;
}
EXPORT("scripts_need") uint32_t scripts_need(uint32_t state) { return (uint32_t)coaxial_scripts_need(at(state), (coax_v_u8){out, sizeof out}); }
EXPORT("scripts_got") void scripts_got(uint32_t state, int32_t status, uint32_t n) {
  if (n > sizeof in) { n = 0; status = 0; }
  coaxial_scripts_got(at(state), status, (coax_v_u8){in, (int64_t)n});
}
EXPORT("scripts_tick") uint32_t scripts_tick(uint32_t state, uint32_t n) {
  if (n > sizeof in) return 0;
  return (uint32_t)coaxial_scripts_tick(at(state), (coax_v_u8){in, (int64_t)n}, (coax_v_u8){out, sizeof out});
}
EXPORT("scripts_status") uint32_t scripts_status(uint32_t state) { return (uint32_t)coaxial_scripts_status(at(state), (coax_v_u8){out, sizeof out}); }
