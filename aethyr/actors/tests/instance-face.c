// Thin projections of the Coax lifecycle trial; no policy or sequencing here.
#include "instance-trial.h"
#include <stdint.h>
#ifdef __wasm__
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
static coax_t_ActorTrial trial;
static coax_v_t_ActorTrial at(void) {return (coax_v_t_ActorTrial){&trial,1};}
EXPORT("trial") int32_t actor_instance_trial(int32_t fps) {return (int32_t)actor_trial_run(at(),fps);}
EXPORT("word") int64_t actor_instance_word(int32_t row,int32_t word) {return actor_trial_word(at(),row,word);}
