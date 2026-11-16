// Firm reason: 1 (scalar Wasm views for the shared GPU fixture; no renderer logic).
#include "gpu-composition-fixture.h"
#include <stdint.h>
#define EXPORT(x) __attribute__((export_name(x)))
static coax_t_UcFixture fixture;
static float vertices[32768];
EXPORT("frame") int frame(int mode,int w,int h) {fixture.mode=mode;return (int)uc_fixture((coax_v_t_UcFixture){&fixture,1},(coax_v_f32){vertices,32768},w,h);}
EXPORT("vertices") uintptr_t vertex_view(void) {return (uintptr_t)vertices;}
EXPORT("plan") uintptr_t plan_view(void) {return (uintptr_t)fixture.plan.e[0].ops.e;}
EXPORT("passes") int passes(void) {return (int)fixture.plan.e[0].n;}
EXPORT("image") uintptr_t image_view(void) {return (uintptr_t)fixture.image.e;}
