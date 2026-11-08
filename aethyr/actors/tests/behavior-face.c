// Thin Wasm ABI projections of the Coax behavior trial; no actor policy here.
#include "behavior-trial.h"
#include <stdint.h>
#ifdef __wasm__
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
static coax_t_BehaviorTrial trial;
static coax_v_t_BehaviorTrial at(void) {return (coax_v_t_BehaviorTrial){&trial,1};}
EXPORT("trial") int32_t actor_behavior_trial(int32_t fps) {return (int32_t)behavior_trial_run(at(),fps);}
EXPORT("word") int64_t actor_behavior_word(int32_t row,int32_t word) {return behavior_trial_word(at(),row,word);}
