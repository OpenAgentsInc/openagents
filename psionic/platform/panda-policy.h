/* Resident Coax GR00T effects. All learned and preparation arithmetic is Coax.
   Sources/PST must be authenticated and immutable for the context lifetime.
   One caller owns a context; calls do not overlap. No CPU model fallback. */
#ifndef PSIONIC_PANDA_POLICY_H
#define PSIONIC_PANDA_POLICY_H
#include <stddef.h>
#include <stdint.h>
typedef struct PandaPolicy PandaPolicy;
/* Every int-returning effect method below returns1 for success,0 for failure.
   This follows the C effect API, not Coax's zero-valued status enums. */
typedef struct {
    const char *shard_directory;
    const char *pst_path;
    const char *instruction; /* ASCII; Coax performs source normalization/BPE */
    /* Coax-generated tile, device, vision, wide, text and action sources. */
    const char *metal_paths[6];
} PandaPolicyConfig;
typedef struct {
    double preparation_seconds, vision_gpu_seconds, text_gpu_seconds;
    double action_gpu_seconds, total_seconds;
    int64_t tokens, launches;
} PandaPolicyTiming;
int panda_policy_open(const PandaPolicyConfig *, PandaPolicy **, char *error, size_t error_capacity);
/* Two genuine rawOpenGL256x256RGB images, original physical state8, explicit
   caller-owned noise5280. Output is16x7 source-decoded commands, atomically. */
int panda_policy_forward(PandaPolicy *, const uint8_t agentview[196608],
    const uint8_t wrist[196608], const float state8[8],
    const float noise5280[5280], float decoded112[112], PandaPolicyTiming *);
const char *panda_policy_error(const PandaPolicy *);
void panda_policy_close(PandaPolicy *);
/* Opt-in diagnostic copies from the most recently completed private frame.
   name: pixels/state/noise/ids/backbone/normalized-actions/decoded-actions. */
int panda_policy_copy(PandaPolicy *, const char *name, void *out, size_t bytes);
/* Development-only captures; caller creates a private directory. NULL disables. */
int panda_policy_capture(PandaPolicy *, const char *directory);
#endif
