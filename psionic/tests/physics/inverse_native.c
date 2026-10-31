/* Firm reason2: native ABI, original-solver state identity and derivative/fit transcripts. */
#include "inverse.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static long assertions;
#define CHECK(x) do {++assertions;if(!(x)){fprintf(stderr,"line %d: %s\n",__LINE__,#x);exit(1);}} while(0)
static uint64_t bits(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static double number(uint64_t u){double x;memcpy(&x,&u,8);return x;}
static FILE* create(const char*prefix,const char*suffix){char p[4096];CHECK(snprintf(p,sizeof p,"%s.%s",prefix,suffix)>0);FILE*f=fopen(p,"wb");CHECK(f);return f;}
static void write_data(FILE*f,coax_t_OiData d){CHECK(fwrite(d.initial.e,sizeof(coax_t_OsState),3,f)==3);CHECK(fwrite(d.forces.e,8,72,f)==72);CHECK(fwrite(d.observed.e,sizeof(coax_t_OsState),72,f)==72);}
static void fit_json(FILE*f,const char*name,coax_t_OiFit fit,coax_t_OiEval eval){fprintf(f,"\"%s\":{\"spring\":%.17g,\"damping\":%.17g,\"loss\":%.17g,\"forward\":%lld,\"reverse\":%lld,\"projected\":%lld,\"rows\":%lld,\"q_square\":%.17g,\"v_square\":%.17g,\"max_q\":%.17g,\"max_v\":%.17g}",name,fit.params.spring,fit.params.damping,fit.loss,(long long)fit.forward,(long long)fit.reverse,(long long)fit.projected,(long long)eval.rows,eval.q_square,eval.v_square,eval.max_q,eval.max_v);}
int main(int argc,char**argv){CHECK(argc==2);FILE*data_file=create(argv[1],"data.f64"),*states_file=create(argv[1],"states.f64"),*gradient_file=create(argv[1],"gradients.json"),*fits_file=create(argv[1],"fits.json");
 const double points[5][2]={{1,.9},{2.4,.55},{3.5,.2},{.5,.1},{4,1}},sweep[]={.01,.001,.0001,.00001,.000001,.0000001};
 fprintf(gradient_file,"[\n");fprintf(fits_file,"[\n");
 for(int mode=0;mode<7;mode++){
  coax_t_OiData data=oi_dataset(mode);CHECK(data.status==0&&data.episodes==3&&data.steps==24);write_data(data_file,data);coax_t_OiProgram program=oi_program(data);CHECK(program.status==0&&program.states==72&&program.observations==(mode%2?18:72)&&program.count==(mode%2?3255:3687));
  for(int point=0;point<5;point++){
   double p[2]={points[point][0],points[point][1]},values[4096];CHECK(tp_forward((coax_v_t_TpNode){program.nodes.e,program.count},(coax_v_f64){p,2},(coax_v_f64){values,4096},0)==0);
   for(int ep=0;ep<3;ep++){coax_t_OsState s=data.initial.e[ep];for(int tick=0;tick<24;tick++){int at=ep*24+tick;coax_t_OsStep step=os_step((coax_t_OsParams){1,p[0],p[1]},s,data.forces.e[at],.05);CHECK(step.status==0);s=step.state;double q=values[program.q_node.e[at]],v=values[program.v_node.e[at]];CHECK(bits(q)==bits(s.position)&&bits(v)==bits(s.velocity));CHECK(fwrite(&q,8,1,states_file)==1&&fwrite(&v,8,1,states_file)==1);}}
   coax_t_OiValue g=oi_value(program,p[0],p[1],1);CHECK(g.status==0&&g.forward&&g.reverse);
   fprintf(gradient_file,"%s{\"mode\":%d,\"spring\":%.17g,\"damping\":%.17g,\"loss\":%.17g,\"gradient\":[%.17g,%.17g],\"fd\":[",(mode||point)?",\n":"",mode,p[0],p[1],g.loss,g.gradient.e[0],g.gradient.e[1]);
   if(point<3){for(int axis=0;axis<2;axis++)for(int level=0;level<6;level++){double up[2]={p[0],p[1]},down[2]={p[0],p[1]};up[axis]+=sweep[level];down[axis]-=sweep[level];coax_t_OiValue a=oi_value(program,up[0],up[1],0),b=oi_value(program,down[0],down[1],0);CHECK(a.status==0&&b.status==0&&a.forward&&b.forward&&!a.reverse&&!b.reverse);double fd=(a.loss-b.loss)/(2*sweep[level]);if(level==3)CHECK(fabs(fd-g.gradient.e[axis])<=1e-7);fprintf(gradient_file,"%s{\"axis\":%d,\"h\":%.17g,\"value\":%.17g}",(axis||level)?",":"",axis,sweep[level],fd);}}
   fprintf(gradient_file,"]}");
  }
  coax_t_OsParams start={1,mode==6?2.4:1,.9};coax_t_OiFit fit=oi_fit(program,start,255),grid=oi_grid(program,16);CHECK(fit.status==0&&grid.status==0&&fit.forward==256&&fit.reverse==255&&grid.forward==256&&grid.reverse==0);
  if(mode==0)CHECK(fabs(fit.params.spring-2.4)<=.02&&fabs(fit.params.damping-.55)<=.02);
  if(mode==6)CHECK(fit.params.spring==2.4&&fit.params.damping==.9&&fit.loss==0);
  coax_t_OiEval learned=oi_forecast(fit.params),searched=oi_forecast(grid.params);CHECK(learned.status==0&&searched.status==0&&learned.rows==384&&searched.rows==384);
  fprintf(fits_file,"%s{\"mode\":%d,\"nodes\":%lld,\"observations\":%lld,",mode?",\n":"",mode,(long long)program.count,(long long)program.observations);fit_json(fits_file,"gradient",fit,learned);fprintf(fits_file,",");fit_json(fits_file,"grid",grid,searched);fprintf(fits_file,"}");
 }
 fprintf(gradient_file,"\n]\n");fprintf(fits_file,"\n]\n");CHECK(fclose(data_file)==0&&fclose(states_file)==0&&fclose(gradient_file)==0&&fclose(fits_file)==0);
 coax_t_OiData original=oi_dataset(0);uint64_t invalid[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000123)};
 for(int slot=0;slot<222;slot++)for(int which=0;which<3;which++){coax_t_OiData d=original;double bad=number(invalid[which]);if(slot<6){if(slot%2)d.initial.e[slot/2].velocity=bad;else d.initial.e[slot/2].position=bad;}else if(slot<78)d.forces.e[slot-6]=bad;else {int n=slot-78;if(n%2)d.observed.e[n/2].velocity=bad;else d.observed.e[n/2].position=bad;}CHECK(oi_data_ok(d)==1);coax_t_OiProgram p=oi_program(d);CHECK(p.status==1&&p.count==0&&p.states==0);}
 coax_t_OiProgram program=oi_program(original);for(int field=0;field<2;field++)for(int which=0;which<3;which++){double k=field?2.4:number(invalid[which]),c=field?number(invalid[which]):.55;coax_t_OiValue g=oi_value(program,k,c,1);CHECK(g.status==1&&!g.forward&&!g.reverse&&g.gradient.e[0]==0&&g.gradient.e[1]==0);}
 FILE*record=create(argv[1],"native.json");fprintf(record,"{\"assertions_before_final_close\":%ld,\"bit_identical_original_states\":2520,\"gradient_cases\":35,\"central_difference_estimates\":252,\"nonfinite_data_cases\":666,\"nonfinite_parameter_cases\":6,\"fit_variants\":7}\n",assertions);CHECK(fclose(record)==0);printf("inverse physics native: %ld assertions; 2520 original states bit-identical; 35 gradients; 252 FD estimates; 7 matched-budget fits\n",assertions);return 0;
}
