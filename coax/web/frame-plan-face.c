// Firm reason: 1 (browser glue: the reactor's two word buffers and one scalar call into Coax's frame plan).
// build/frame-plan.wasm (GitHub #13451): coax/web/frame-plan-host.js writes a page's counts to the ask
// and reads the frame's operations from the plan. metaverse-engine/render/graph/frame-plan.coax states both.
#include "frame-plan-carrier.h"
#define FRAME_PLAN_EXPORT(name) __attribute__((export_name(name)))
#define PLAN_WORDS (257 * 8)
static uint32_t ask[64];
static uint32_t plan[PLAN_WORDS];
FRAME_PLAN_EXPORT("frame_plan_ask_ptr") uint32_t frame_plan_ask_ptr(void) { return (uint32_t)(uintptr_t)ask; }
FRAME_PLAN_EXPORT("frame_plan_ask_words") uint32_t frame_plan_ask_words(void) { return (uint32_t)frame_plan_carrier_words(); }
FRAME_PLAN_EXPORT("frame_plan_ptr") uint32_t frame_plan_ptr(void) { return (uint32_t)(uintptr_t)plan; }
FRAME_PLAN_EXPORT("frame_plan_capacity") uint32_t frame_plan_capacity(void) { return PLAN_WORDS; }
/* The words written to the plan; 0 when the ask is refused. */
FRAME_PLAN_EXPORT("frame_plan_write") uint32_t frame_plan_write_v1(void) {
  int64_t words = frame_plan_carrier_words();
  if (words < 1 || words > (int64_t)(sizeof ask / sizeof ask[0])) return 0u;
  return (uint32_t)frame_plan_carrier_ask((coax_v_u32){ask, words}, (coax_v_u32){plan, PLAN_WORDS});
}
