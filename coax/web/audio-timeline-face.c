// Firm reason: 1 (browser SDK: scalar calls into the shared Coax clip policy).
#include "audio-timeline.h"
#include <stdint.h>
#define CLIP_EXPORT(name) __attribute__((export_name(name)))
static coax_t_HostAudioTimeline clip_state;
static coax_t_HostAudioClipPlan clip_output;
static coax_v_t_HostAudioTimeline clip_view(void){return (coax_v_t_HostAudioTimeline){&clip_state,1};}
CLIP_EXPORT("clip_reset") void clip_reset(void){coax_default_t_HostAudioTimeline(&clip_state);coax_default_t_HostAudioClipPlan(&clip_output);}
CLIP_EXPORT("clip_offer") uint32_t clip_offer(int64_t us,int64_t bytes){return hcl_offer(clip_view(),us,bytes);}
CLIP_EXPORT("clip_seal") uint32_t clip_seal(void){return hcl_seal(clip_view());}
CLIP_EXPORT("clip_boundary") int64_t clip_boundary(int64_t us){return hcl_boundary(us);}
CLIP_EXPORT("clip_plan") uint32_t clip_plan(int64_t start,int64_t duration){clip_output=hcl_plan(start,duration);return clip_output.valid;}
CLIP_EXPORT("clip_info") int64_t clip_info(uint32_t field){switch(field){case 0:return clip_state.count;case 1:return clip_state.bytes;case 2:return clip_state.failed;case 3:return clip_output.start;case 4:return clip_output.frames;case 5:return clip_output.total;case 6:return clip_state.sealed;default:return -1;}}
