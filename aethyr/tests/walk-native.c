/* Independent native client of the public, generated Player C header.
 * Every scalar leaf of the pre-extraction Player is serialized by its bits.
 * Producer-loss controls affect current execution only, before serialization. */
#include "player-native.h"
#include <inttypes.h>
#include <math.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define FLOAT_FIELDS(X) \
 X(pos.x) X(pos.y) X(pos.z) X(vel.x) X(vel.y) X(vel.z) X(face) \
 X(theta) X(phi) X(dist) X(max_dist) X(zoom_push) \
 X(target.x) X(target.y) X(target.z) X(spring_vel.x) X(spring_vel.y) X(spring_vel.z) \
 X(key_x) X(key_y) X(pad_x) X(pad_y) \
 X(prev_pos.x) X(prev_pos.y) X(prev_pos.z) X(prev_target.x) X(prev_target.y) X(prev_target.z) \
 X(prev_face) X(prev_dist) X(pace) X(grip) X(collision_residual)
#define INTEGER_FIELDS(X) \
 X(on_ground) X(act) X(run) X(jump_held) \
 X(jumps.held) X(jumps.waiting) X(jumps.pressed) X(jumps.taken) X(jumps.lost) \
 X(clock.step_ns) X(clock.max_frame_ns) X(clock.max_steps) X(clock.carry_ns) X(clock.due) \
 X(clock.ticks) X(clock.frames) X(clock.refused) X(clock.overlong) X(clock.stalls) \
 X(clock.dropped_ns) X(clock.counted_ns) X(clock.ran) X(sliding) \
 X(collision_status) X(collision_refusals) X(collision_passes) X(collision_floor_cut) \
 X(query_status) X(camera_query_status)

static void schema(void){
 printf("abi %zu %zu",sizeof(coax_t_Player),_Alignof(coax_t_Player));
 #define OFFSET(field) printf(" %s:%zu",#field,offsetof(coax_t_Player,field));
 FLOAT_FIELDS(OFFSET) INTEGER_FIELDS(OFFSET)
 #undef OFFSET
 putchar('\n');
 printf("fields");
 #define FIELD(field) printf(" %s",#field);
 FLOAT_FIELDS(FIELD) INTEGER_FIELDS(FIELD)
 #undef FIELD
 putchar('\n');
}
static void emit(int kind,int frame,coax_t_Player p){
 printf("state %d %d",kind,frame);
 #define FLOAT_WORD(field) do{float value=p.field;uint32_t bits;memcpy(&bits,&value,sizeof bits);printf(" %" PRIu32,bits);}while(0);
 #define INTEGER_WORD(field) printf(" %" PRIu64,(uint64_t)p.field);
 FLOAT_FIELDS(FLOAT_WORD) INTEGER_FIELDS(INTEGER_WORD)
 #undef FLOAT_WORD
 #undef INTEGER_WORD
 putchar('\n');
}
static float duration(int frame){
 if(frame==0)return 0;
 if(frame==1)return .006f;
 if(frame==2)return .004f;
 if(frame==3)return .011625f;
 if(frame==4)return .125f;
 if(frame==5)return .4f;
 if(frame==6)return NAN;
 if(frame==7)return -1;
 if(frame==8)return INFINITY;
 return frame%3==0?1.0f/60:frame%3==1?1.0f/30:1.0f/144;
}
static void producer_loss(coax_t_Player *p,const char *fault,int kind,int frame){
 if(!strcmp(fault,"queued-jump")&&frame==0)p->jumps.waiting=0;
 if(!strcmp(fault,"clock-carry")&&frame==0)p->clock.carry_ns=0;
 if(!strcmp(fault,"counter-high")&&frame==0)p->collision_refusals&=UINT32_MAX;
 if(!strcmp(fault,"previous-pose")&&frame==1)p->prev_pos=p->pos;
 if(!strcmp(fault,"residual")&&kind==4&&frame==1)p->collision_residual=0;
 if(!strcmp(fault,"query-status")&&kind==5&&frame==1)p->query_status=0;
 if(!strcmp(fault,"camera-status")&&frame==0)p->camera_query_status=0;
}
int main(int argc,char **argv){
 if(argc>1&&!strcmp(argv[1],"--assert-probe")){walk_parity_assert_probe(false);return 99;}
 const char *fault=argc>1?argv[1]:"none";schema();
  coax_t_Player constructed;coax_default_t_Player(&constructed);emit(-1,-1,constructed);
 for(int kind=0;kind<8;++kind){
  coax_t_Player p=verse_player_place(kind==4?.4f:0,kind==6?2.5f:0,0,0);
  p.on_ground=true;p.clock.carry_ns=10000000;
  p.collision_status=2;p.collision_refusals=(UINT64_C(1)<<40)|7;
  p.collision_residual=.125f;p.collision_passes=7;p.collision_floor_cut=true;
  p.query_status=2;p.camera_query_status=3;
  if(kind==3){p.grip=.2f;p.pace=.5f;}
  emit(kind,-1,p);
  for(int frame=0;frame<96;++frame){
   bool run=frame%40<24,jump=frame%32<3;
   p=verse_player_input(p,frame%19<6?.4f:0,frame<80?1:0,0,0,run,jump,
                        frame<60?.013f:-.007f,frame%17==0?.003f:0);
   if(frame==20)p=verse_player_zoom(p,.75f);
   p=walk_parity_frame(kind,p,duration(frame));
   producer_loss(&p,fault,kind,frame);emit(kind,frame,p);
  }
 }
 return 0;
}
