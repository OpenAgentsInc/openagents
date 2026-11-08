// Fixture SDK effects/projection only. All gameplay, layout and clocks are Coax.
#include "lab.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_ActorLab lab;
static uint8_t input[8192], hash[64];
static coax_v_t_ActorLab at(void) { return (coax_v_t_ActorLab){&lab,1}; }
static uint32_t address(const void *p) { return (uint32_t)(uintptr_t)p; }
EXPORT("lab_open") int32_t lab_open(int32_t credits) { return actor_lab_start(at(),credits); }
EXPORT("lab_frame") void lab_frame(int64_t ns) { actor_lab_frame(at(),ns); }
EXPORT("lab_script") void lab_script(int32_t on,int32_t observe) { (lab.script=on!=0,lab.observe=observe!=0,lab.trace_n=0); }
EXPORT("lab_trial") int32_t lab_trial(int32_t fps,int32_t visual,int32_t observe) { return actor_lab_trial(at(),fps,visual!=0,observe!=0); }
EXPORT("lab_steps") int32_t lab_steps(int32_t count) { return actor_lab_steps(at(),count); }
EXPORT("lab_action") void lab_action(int32_t action) { actor_lab_action(at(),action); }
EXPORT("lab_key") void lab_key(int32_t length,int32_t down) { actor_lab_key(at(),(coax_string){(const char*)input,length},down!=0); }
EXPORT("lab_view") void lab_view(float width,float height) { actor_lab_view(at(),width,height); }
EXPORT("lab_hit") void lab_hit(float x,float y) { actor_lab_hit(at(),x,y); }
EXPORT("lab_draw_ptr") uint32_t lab_draw_ptr(void) { return address(lab.world.e[0].rects.e); }
EXPORT("lab_draw_count") int32_t lab_draw_count(void) { return (int32_t)lab.world.e[0].rect_n; }
EXPORT("lab_text_ptr") uint32_t lab_text_ptr(int32_t i) { return address(actor_lab_text(at(),i).data); }
EXPORT("lab_text_len") int32_t lab_text_len(int32_t i) { return (int32_t)actor_lab_text(at(),i).count; }
EXPORT("lab_metric_count") int32_t lab_metric_count(void) { return (int32_t)actor_lab_metric_count(); }
EXPORT("lab_metric_name_ptr") uint32_t lab_metric_name_ptr(int32_t i) { return address(actor_lab_metric_name(i).data); }
EXPORT("lab_metric_name_len") int32_t lab_metric_name_len(int32_t i) { return (int32_t)actor_lab_metric_name(i).count; }
EXPORT("lab_metric") double lab_metric(int32_t i) { return actor_lab_metric(at(),i); }
EXPORT("lab_trace_count") int32_t lab_trace_count(void) { return (int32_t)lab.trace_n; }
EXPORT("lab_trace_metric") double lab_trace_metric(int32_t r,int32_t i) { return lab.trace_metrics.e[r].e[i]; }
EXPORT("lab_hash_ptr") uint32_t lab_hash_ptr(void) { actor_lab_hash(at(),(coax_v_u8){hash,64});return address(hash); }
EXPORT("lab_trace_hash_ptr") uint32_t lab_trace_hash_ptr(int32_t r) { actor_lab_trace_hash(at(),r,(coax_v_u8){hash,64});return address(hash); }
EXPORT("lab_input_ptr") uint32_t lab_input_ptr(void) { return address(input); }
EXPORT("lab_import") int32_t lab_import(int32_t length) { return state_lab_import((coax_v_t_StateLab){lab.world.e,1},(coax_v_u8){input,length}); }
EXPORT("lab_saved_ptr") uint32_t lab_saved_ptr(void) { return address(lab.world.e[0].saved.e); }
EXPORT("lab_saved_len") int32_t lab_saved_len(void) { return (int32_t)lab.world.e[0].saved_n; }
EXPORT("lab_saved_version") int32_t lab_saved_version(void) { return (int32_t)lab.world.e[0].saved_version; }

EXPORT("lab_contract_ptr") uint32_t lab_contract_ptr(int32_t fps) {actor_contract_hex(fps,(coax_v_u8){hash,64});return address(hash);}

EXPORT("lab_source") int32_t lab_source(int32_t ai) {return actor_lab_source(at(),ai!=0);}

EXPORT("lab_control_count") int32_t lab_control_count(void) {return actor_lab_control_count(at());}
EXPORT("lab_control_word") double lab_control_word(int32_t row,int32_t column) {return (double)actor_lab_control_word(at(),row,column);}
