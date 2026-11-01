/* NEW own-plant coupled rollout plus a recorded-command own-plant shadow.
   This binary has no SDK dependency: it reads the one captured plant trace. */
#include "panda_joint_pd_prefix_format.h"
typedef struct {PprRecord fresh;double shadow_q[9],shadow_v[9];int64_t force_clipped,shadow_force_clipped;} PprOwnRecord;
_Static_assert(sizeof(PprOwnRecord)==600,"canonical own rollout record");
static void ppr_integrate(double q[9],double v[9],const double controls[9],coax_t_PeuOutput *out) {
 double external[9]={0};memset(out,0xA5,sizeof *out);
 PPR_NEED(peu_step_raw((coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_f64){(double*)controls,9},(coax_v_f64){external,9},(coax_v_t_PeuOutput){out,1})==0);
 PPR_NEED(out->control_clipped==0&&out->force_clipped>=0&&out->force_clipped<=2);
 for(int i=0;i<7;i++){q[i]=out->angles.e[i];v[i]=out->hinge_rates.e[i];}for(int i=0;i<2;i++){q[7+i]=out->slides.e[i];v[7+i]=out->slide_rates.e[i];}ppr_state(q,v);
}
static double ppr_error(const double q[9],const double target[7]) {double total=0;for(int i=0;i<7;i++){double e=target[i]-q[i];total+=e*e;}return sqrt(total);}
int main(int argc,char **argv) {
 PPR_NEED(argc==4);uint16_t endian=1;PPR_NEED(*(uint8_t*)&endian==1);PprInput input=ppr_input(argv[1]);
 FILE *f=fopen(argv[2],"rb");PPR_NEED(f);uint32_t header[2];PPR_NEED(fread(header,1,8,f)==8&&header[0]==UINT32_C(0x31525050)&&header[1]==PPR_STEPS);
 PprRecord reference[PPR_STEPS];PPR_NEED(fread(reference,1,sizeof reference,f)==sizeof reference&&fgetc(f)==EOF&&!fclose(f));
 for(int row=0;row<PPR_STEPS;row++){for(int i=0;i<PPR_WORDS;i++)PPR_NEED(isfinite(reference[row].values[i]));PPR_NEED(reference[row].clipped>=0&&reference[row].clipped<=7);}
 FILE *output=fopen(argv[3],"wbx");PPR_NEED(output);double q[9],v[9],sq[9],sv[9];memcpy(q,input.q,sizeof q);memcpy(v,input.v,sizeof v);memcpy(sq,q,sizeof q);memcpy(sv,v,sizeof v);double time=0,maxq[2]={0},maxv[2]={0},maxcommand=0,maxfield=0,shadow_maxq[2]={0},shadow_maxv[2]={0};
 double initial=ppr_error(q,input.target),minimum=initial,maximum=initial;
 for(int step=0;step<PPR_STEPS;step++) {
  PprRecord ref=reference[step];coax_t_PjpOutput pd=ppr_control(&input,q,v);double controls[9],shadow_controls[9];ppr_controls(&input,pd,controls);for(int i=0;i<7;i++)shadow_controls[i]=ref.values[46+i];shadow_controls[7]=input.finger[0];shadow_controls[8]=input.finger[1];
  coax_t_PeuOutput integrated,shadow;ppr_integrate(q,v,controls,&integrated);ppr_integrate(sq,sv,shadow_controls,&shadow);time+=0.002;PprOwnRecord record;ppr_pack(&record.fresh,q,v,pd,time);memcpy(record.shadow_q,sq,sizeof sq);memcpy(record.shadow_v,sv,sizeof sv);record.force_clipped=integrated.force_clipped;record.shadow_force_clipped=shadow.force_clipped;
  PPR_NEED(pd.clipped==ref.clipped&&fabs(time-ref.values[53])<=1e-12);
  for(int i=0;i<9;i++) {
   int group=i<7?0:1;double qe=fabs(q[i]-ref.values[i]),ve=fabs(v[i]-ref.values[9+i]),sqe=fabs(sq[i]-ref.values[i]),sve=fabs(sv[i]-ref.values[9+i]);PPR_NEED(isfinite(qe)&&isfinite(ve)&&isfinite(sqe)&&isfinite(sve));
   PPR_NEED(qe<=(group?1e-9:1e-8)&&ve<=(group?1e-8:1e-7)&&sqe<=(group?1e-9:1e-8)&&sve<=(group?1e-8:1e-7));
   if(qe>maxq[group])maxq[group]=qe;if(ve>maxv[group])maxv[group]=ve;if(sqe>shadow_maxq[group])shadow_maxq[group]=sqe;if(sve>shadow_maxv[group])shadow_maxv[group]=sve;
  }
  for(int i=0;i<35;i++){double error=fabs(record.fresh.values[18+i]-ref.values[18+i]);PPR_NEED(isfinite(error)&&error<=1e-6);if(error>maxfield)maxfield=error;if(i>=28&&error>maxcommand)maxcommand=error;}
  double error=ppr_error(q,input.target);PPR_NEED(isfinite(error));if(error<minimum)minimum=error;if(error>maximum)maximum=error;PPR_NEED(fwrite(&record,1,sizeof record,output)==sizeof record);
 }
 PPR_NEED(!fclose(output));
 printf("{\"passed\":true,\"SDK_calls\":0,\"steps_per_own_branch\":250,\"own_branches\":2,\"assertions\":%zu,\"record_bytes\":600,\"max_q_rad_m\":[%.17g,%.17g],\"max_v_rad_s_m_s\":[%.17g,%.17g],\"shadow_max_q_rad_m\":[%.17g,%.17g],\"shadow_max_v_rad_s_m_s\":[%.17g,%.17g],\"max_command_error_J_per_rad\":%.17g,\"max_PD_field_error_J_per_rad\":%.17g,\"initial_error_rad\":%.17g,\"final_error_rad\":%.17g,\"minimum_error_rad\":%.17g,\"maximum_error_rad\":%.17g}\n",ppr_checks,maxq[0],maxq[1],maxv[0],maxv[1],shadow_maxq[0],shadow_maxq[1],shadow_maxv[0],shadow_maxv[1],maxcommand,maxfield,initial,ppr_error(q,input.target),minimum,maximum);return 0;
}
