/* Test-only consumer: a real wasm env import plus the production Coax face. */
#include <stdint.h>
#ifdef STDIO_PROBE
#include <stdio.h>
#endif
#ifndef OMIT_ENTROPY_FACE
#define COAX_ENTROPY_HEADER "entropy_import.h"
#include "../platform/entropy-wasm.c"
#else
#define EXPORT(name) __attribute__((export_name(name)))
#endif
#ifdef UNKNOWN_RANDOM_IMPORT
__attribute__((import_module("wasi_snapshot_preview1"),import_name("random_get")))
#elif defined(UNKNOWN_PROTO_IMPORT)
__attribute__((import_module("env"),import_name("toString")))
#else
__attribute__((import_module("env"),import_name("coaxial_random")))
#endif
extern uint32_t consume_random(uint32_t out,int32_t length);
#ifndef PROBE_STORAGE
#define PROBE_STORAGE 1048584
#endif
static uint8_t output[PROBE_STORAGE],inbox[4096];
static uint32_t initialized=99;
__attribute__((constructor)) static void initialize_probe(void){
#ifdef STDIO_PROBE
  puts("test-only libc probe");fflush(stdout);
#endif
  initialized=consume_random((uint32_t)(uintptr_t)output,1);
}
EXPORT("probe_random") uint32_t probe_random(int32_t n){return consume_random((uint32_t)(uintptr_t)output,n);}
EXPORT("probe_at") uint32_t probe_at(uint32_t p,int32_t n){return consume_random(p,n);}
EXPORT("probe_output") uint32_t probe_output(void){return (uint32_t)(uintptr_t)output;}
EXPORT("coaxial_layout") uint32_t layout(void){return 1;}
EXPORT("coaxial_state_size") uint32_t state_size(void){return 4;}
EXPORT("coaxial_max_vertices") uint32_t vertices(void){return 0;}
EXPORT("coaxial_vertex_floats") uint32_t floats(void){return 4;}
EXPORT("coaxial_uniform_floats") uint32_t uniforms(void){return 4;}
EXPORT("coaxial_inbox") uint32_t input(uint32_t state){(void)state;return (uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t input_cap(void){return sizeof inbox;}
EXPORT("coaxial_message") void message(uint32_t state,uint32_t kind,uint32_t length){
  (void)length;
  if(kind==999){
    output[32]=77;
    *(uint32_t *)(uintptr_t)state=consume_random((uint32_t)(uintptr_t)output,32)==0&&initialized==0&&output[32]==77?901:902;
  }
}
EXPORT("coaxial_action") uint32_t action(uint32_t state){return *(uint32_t *)(uintptr_t)state;}
EXPORT("coaxial_action_done") void done(uint32_t state){*(uint32_t *)(uintptr_t)state=0;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t state){(void)state;return 0;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t state){(void)state;return 0;}
