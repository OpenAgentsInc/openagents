/* Memory and opaque-handle effects only; all admission/audio decisions Coax. */
#include "dictation.h"
#include "dictation-face.h"
#include <stdlib.h>
#include <string.h>
typedef struct {coax_t_DctState state;float clip[960000];} DctOwner;
static coax_v_t_DctState view(void *p){return (coax_v_t_DctState){&((DctOwner*)p)->state,1};}
void *dct_new(void){DctOwner*p=calloc(1,sizeof *p);if(!p)return NULL;coax_default_t_DctState(&p->state);p->state.clip=(coax_v_f32){p->clip,960000};return p;}
void dct_free(void*p){if(p){memset(p,0,sizeof(DctOwner));free(p);}}
void dct_enable(void*p,int on,int64_t now){(void)now;if(p)dct_set_enabled(view(p),on!=0);}
void dct_active(void*p,int on){if(p)dct_set_active(view(p),on!=0);}
void dct_ready(void*p,int on){if(p)dct_set_ready(view(p),on!=0);}
void dct_preparing(void*p,int reason){if(p)dct_prepare(view(p),reason);}
int dct_enabled(void*p){return p?((DctOwner*)p)->state.gate.enabled:0;}
int dct_model_ready(void*p){return p?((DctOwner*)p)->state.gate.ready:0;}
int dct_is_active(void*p){return p?((DctOwner*)p)->state.gate.active:0;}
int dct_capture_allowed(void*p){return p?dct_allow(view(p)):0;}
int dct_preparing_reason(void*p){return p?(int)((DctOwner*)p)->state.gate.pending:0;}
uint64_t dct_begin(void*p,int64_t now){return p?dct_start(view(p),now):0;}
void dct_release(void*p,int64_t now){if(p)dct_end(view(p),now);}
void dct_cancel(void*p){if(p)dct_abort(view(p));}
void dct_fault(void*p,int reason){if(p)dct_failed(view(p),reason);}
void dct_audio(void*p,const float*x,int32_t n,int32_t rate,int64_t start){if(p)dct_audio_feed(view(p),(coax_v_f32){(float*)x,x?n:0},rate,start);}
void dct_tick(void*p,int64_t now){if(p)dct_clock(view(p),now);}
int dct_phase(void*p){return p?(int)((DctOwner*)p)->state.gate.phase:0;}
int dct_outcome(void*p){return p?(int)((DctOwner*)p)->state.gate.outcome:0;}
int dct_fault_reason(void*p){return p?(int)((DctOwner*)p)->state.gate.fault:0;}
uint64_t dct_ticket(void*p){return p?((DctOwner*)p)->state.gate.ticket:0;}
int32_t dct_samples(void*p){return p?(int32_t)((DctOwner*)p)->state.n:0;}
int32_t dct_segment(void*p,int32_t offset){return p?(int32_t)dct_next_segment(view(p),offset):0;}
int32_t dct_segment_leading_silence(void*p,int32_t offset){return p?(int32_t)dct_segment_lead(view(p),offset):0;}
int32_t dct_segment_trailing_silence(void*p,int32_t offset){return p?(int32_t)dct_segment_trail(view(p),offset):0;}
int32_t dct_copy_audio(void*p,int32_t offset,float*out,int32_t cap){return p?(int32_t)dct_copy_samples(view(p),offset,(coax_v_f32){out,out?cap:0}):-1;}
int dct_decode_begin(void*p,uint64_t ticket){return p?dct_begin_decode(view(p),ticket):0;}
int dct_result(void*p,uint64_t ticket,int success){return p?dct_complete(view(p),ticket,success!=0):0;}
int dct_append_text(void*p,uint64_t ticket,const uint8_t*text,int32_t n){return p?dct_add_text(view(p),ticket,(coax_string){(const char*)text,text?n:0}):0;}
int32_t dct_text_count(void*p){return p?(int32_t)((DctOwner*)p)->state.text_n:0;}
int32_t dct_copy_text(void*p,uint8_t*out,int32_t cap){return p?(int32_t)dct_copy_words(view(p),(coax_v_u8){out,out?cap:0}):-1;}
int dct_device_matches(const uint8_t*w,int32_t wn,const uint8_t*u,int32_t un,int d){if((!w&&wn!=0)||(!u&&un!=0))return 0;return dci_matches((coax_string){(const char*)w,wn},(coax_string){(const char*)u,un},d!=0);}
int dct_device_format(double r,int64_t c,int64_t s,int h){return dci_format(r,c,s,h!=0);}
