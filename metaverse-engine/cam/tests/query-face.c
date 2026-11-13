// Thin ABI storage and projection; all simulation and query logic is Coax.
#include "query-trial.h"
#include <stdint.h>
static coax_t_CameraTrial trial_state;
#ifdef __wasm__
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
EXPORT("trial") int32_t camera_trial(int32_t fps) {
  return (int32_t)camera_trial_run((coax_v_t_CameraTrial){&trial_state,1},fps);
}
EXPORT("word") int64_t camera_word(int32_t row,int32_t word) {
  return camera_trial_word((coax_v_t_CameraTrial){&trial_state,1},row,word);
}
