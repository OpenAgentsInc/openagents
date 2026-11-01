/* Fixed little-endian private experiment transport; not a product protocol. */
#ifndef PSIONIC_PANDA_JOINT_PD_PREFIX_FORMAT_H
#define PSIONIC_PANDA_JOINT_PD_PREFIX_FORMAT_H
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "panda-pd-session.h"
static size_t ppr_checks;
#define PPR_NEED(x) do {ppr_checks++;if(!(x)){fprintf(stderr,"PD prefix line%d: %s\n",__LINE__,#x);exit(2);}}while(0)
enum { PPR_STEPS=250,PPR_WORDS=54 };
typedef struct {double q[9],v[9],target[7],target_v[7],kp[7],kd[7],finger[2];} PprInput;
typedef struct {double values[PPR_WORDS];int64_t clipped;} PprRecord;
_Static_assert(sizeof(PprInput)==48*8&&sizeof(PprRecord)==440,"frozen transport layout");
_Static_assert(sizeof(coax_t_PjpOutput)==288&&sizeof(coax_t_PeuOutput)==232,"qualified output ABI");
static PprInput ppr_input(const char *path) {
 FILE *f=fopen(path,"rb");PPR_NEED(f);uint32_t header[2];PprInput result;
 PPR_NEED(fread(header,1,8,f)==8&&header[0]==UINT32_C(0x31495050)&&header[1]==48);
 PPR_NEED(fread(&result,1,sizeof result,f)==sizeof result&&fgetc(f)==EOF&&!fclose(f));
 double words[48];memcpy(words,&result,sizeof words);for(int i=0;i<48;i++)PPR_NEED(isfinite(words[i]));return result;
}
static coax_t_PjpOutput ppr_control(const PprInput *input,const double q[9],const double v[9]) {
 double words[44];memcpy(words,q,9*8);memcpy(words+9,v,7*8);memcpy(words+16,input->target,7*8);memcpy(words+23,input->target_v,7*8);memcpy(words+30,input->kp,7*8);memcpy(words+37,input->kd,7*8);
 coax_t_PjpOutput output;memset(&output,0xA5,sizeof output);
 PPR_NEED(pjp_forward_raw((coax_v_f64){words,44},(coax_v_t_PjpOutput){&output,1})==0);
 double fields[35];memcpy(fields,&output,35*8);for(int i=0;i<35;i++)PPR_NEED(isfinite(fields[i]));PPR_NEED(output.clipped>=0&&output.clipped<=7);return output;
}
static void ppr_controls(const PprInput *input,coax_t_PjpOutput pd,double controls[9]) {for(int i=0;i<7;i++)controls[i]=pd.command.e[i];controls[7]=input->finger[0];controls[8]=input->finger[1];}
static void ppr_state(const double q[9],const double v[9]) {for(int i=0;i<9;i++)PPR_NEED(isfinite(q[i])&&isfinite(v[i])&&fabs(q[i])<=(i<7?100:1)&&fabs(v[i])<=(i<7?20:2));}
static void ppr_pack(PprRecord *record,const double q[9],const double v[9],coax_t_PjpOutput pd,double time) {memcpy(record->values,q,9*8);memcpy(record->values+9,v,9*8);memcpy(record->values+18,&pd,35*8);record->values[53]=time;record->clipped=pd.clipped;for(int i=0;i<PPR_WORDS;i++)PPR_NEED(isfinite(record->values[i]));}
#endif
