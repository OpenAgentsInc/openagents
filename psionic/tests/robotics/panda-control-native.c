/* Independent FFI/effect harness: no controller algebra implemented here.
   Prepared F64 cases invoke actual Coax reset/step and dump bounded results. */
#include "panda-control.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned cases;
static coax_v_t_PcState view(coax_t_PcState *s){return(coax_v_t_PcState){s,1};}
static void read_exact(FILE *f,void *p,size_t n){assert(fread(p,1,n,f)==n);}
static void write_exact(FILE *f,const void *p,size_t n){assert(fwrite(p,1,n,f)==n);}
static void refused(coax_t_PcState *s,const coax_t_PcState *old,coax_t_PcOutput out){assert(out.status!=0);assert(!memcmp(s,old,sizeof *s));for(int i=0;i<7;i++)assert(out.torque.e[i]==0);for(int i=0;i<2;i++)assert(out.gripper.e[i]==0);cases++;}
static void abi(coax_t_PcSample original){coax_a7_f64 home=pc_default_home();coax_t_PcState state={0};assert(!pc_reset(view(&state),original,(coax_v_f64){home.e,7}));coax_t_PcState saved=state;double action[7]={0};const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000001),UINT64_C(0x7ff0000000000001)};
 for(int field=0;field<144;field++)for(int kind=0;kind<4;kind++){coax_t_PcSample sample=original;memcpy((unsigned char*)&sample+field*8,&bad[kind],8);refused(&state,&saved,pc_step(view(&state),sample,(coax_v_f64){action,7},1));}
 for(int field=0;field<7;field++)for(int kind=0;kind<4;kind++){memcpy(action+field,&bad[kind],8);refused(&state,&saved,pc_step(view(&state),original,(coax_v_f64){action,7},1));action[field]=0;}
 for(int field=0;field<21;field++)for(int kind=0;kind<4;kind++){state=saved;memcpy((unsigned char*)&state+offsetof(coax_t_PcState,goal_position)+field*8,&bad[kind],8);coax_t_PcState before=state;refused(&state,&before,pc_step(view(&state),original,(coax_v_f64){action,7},1));}state=saved;
 for(int field=0;field<7;field++)for(int kind=0;kind<4;kind++){coax_a7_f64 corrupted=home;memcpy(corrupted.e+field,&bad[kind],8);assert(pc_reset(view(&state),original,(coax_v_f64){corrupted.e,7})==COAX_PcStatus_number);assert(!memcmp(&state,&saved,sizeof state));cases++;}
 assert(pc_step((coax_v_t_PcState){&state,0},original,(coax_v_f64){action,7},1).status==COAX_PcStatus_shape);assert(pc_step(view(&state),original,(coax_v_f64){action,6},1).status==COAX_PcStatus_shape);assert(!memcmp(&state,&saved,sizeof state));cases+=2;
 for(int kind=0;kind<4;kind++){coax_t_PcSample sample=original;if(kind==0)sample.mass.e[0]=0;if(kind==1)sample.mass.e[1]+=1;if(kind==2)sample.rotation.e[0]=3;if(kind==3)sample.upper.e[0]=81;refused(&state,&saved,pc_step(view(&state),sample,(coax_v_f64){action,7},1));}
 coax_a49_f64 matrix={0};for(int i=0;i<7;i++)matrix.e[i*7+i]=1;for(int field=0;field<49;field++){coax_a49_f64 corrupted=matrix;memcpy(corrupted.e+field,bad,8);coax_t_PmResult a=pm_inverse(7,corrupted),b=pm_pinv(6,corrupted);assert(a.status==COAX_PmCode_number&&b.status==COAX_PmCode_number);for(int i=0;i<49;i++)assert(a.value.e[i]==0&&b.value.e[i]==0);cases++;}
}
int main(int argc,char **argv){assert(argc==5);FILE *input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx");assert(input&&output);uint32_t magic,count;read_exact(input,&magic,4);read_exact(input,&count,4);assert(magic==0x50433136&&count<=4096);coax_t_PcSample first={0};
 for(uint32_t case_id=0;case_id<count;case_id++){coax_t_PcSample sample;double home[7],prime[7],action[7];uint32_t use_prime,ticks;read_exact(input,&sample,sizeof sample);if(case_id==0)first=sample;read_exact(input,home,sizeof home);read_exact(input,prime,sizeof prime);read_exact(input,action,sizeof action);read_exact(input,&use_prime,4);read_exact(input,&ticks,4);assert(use_prime<=1&&ticks>=1&&ticks<=25);coax_t_PcState state={0};assert(pc_reset(view(&state),sample,(coax_v_f64){home,7})==COAX_PcStatus_ok);if(use_prime){coax_t_PcOutput initial=pc_step(view(&state),sample,(coax_v_f64){prime,7},1);assert(!initial.status);}coax_t_PcOutput r={0};for(uint32_t tick=0;tick<ticks;tick++){r=pc_step(view(&state),sample,(coax_v_f64){action,7},tick==0);if(r.status){fprintf(stderr,"controller case%u tick%u refused status%u\n",case_id,tick,r.status);return 1;}}
  uint32_t status=r.status;write_exact(output,&status,4);write_exact(output,r.torque.e,7*8);write_exact(output,r.gripper.e,2*8);write_exact(output,state.goal_position.e,3*8);write_exact(output,state.goal_rotation.e,9*8);write_exact(output,state.grip.e,2*8);write_exact(output,state.home.e,7*8);uint32_t clips[2]={(uint32_t)r.torque_clipped,(uint32_t)r.action_clipped};write_exact(output,clips,8);cases++;
 }assert(fgetc(input)==EOF);assert(!fclose(input)&&!fclose(output));abi(first);
 input=fopen(argv[3],"rb");output=fopen(argv[4],"wbx");assert(input&&output);read_exact(input,&magic,4);read_exact(input,&count,4);assert(magic==0x504d3136&&count<=4096);for(uint32_t i=0;i<count;i++){uint32_t n,operation;coax_a49_f64 a;read_exact(input,&n,4);read_exact(input,&operation,4);read_exact(input,a.e,49*8);assert(n>=1&&n<=7&&operation<=1);coax_t_PmResult r=operation?pm_pinv(n,a):pm_inverse(n,a);uint32_t status=r.status,rank=(uint32_t)r.rank;write_exact(output,&status,4);write_exact(output,&rank,4);write_exact(output,r.value.e,49*8);cases++;}assert(fgetc(input)==EOF);assert(!fclose(input)&&!fclose(output));printf("Panda controller/matrix native: %u frozen source-input/numeric/refusal cases passed\n",cases);return 0;
}
