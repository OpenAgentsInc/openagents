/* Test-only ABI/effects witness. Coax owns admission, timing and sampling. */
#include "panda-quintic.h"
#include <float.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t assertions,aliases,endpoints,tick_refusals,sample_count,duration_refusals;
static double lower[7],upper[7];
#define NEED(x) do{assertions++;if(!(x)){fprintf(stderr,"Quintic native line%d: %s\n",__LINE__,#x);exit(2);}}while(0)
_Static_assert(sizeof(double)==8&&DBL_MANT_DIG==53&&FLT_RADIX==2,"binary64");
_Static_assert(sizeof(coax_t_PqtTiming)==32&&offsetof(coax_t_PqtTiming,steps)==0&&offsetof(coax_t_PqtTiming,total_nanoseconds)==8&&offsetof(coax_t_PqtTiming,duration)==16&&offsetof(coax_t_PqtTiming,required_estimate)==24,"timing ABI");
_Static_assert(sizeof(coax_t_PqtSample)==168&&offsetof(coax_t_PqtSample,angle)==0&&offsetof(coax_t_PqtSample,rate)==56&&offsetof(coax_t_PqtSample,acceleration)==112,"sample ABI");
static coax_t_PqtStatus(*const typed_time)(coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_t_PqtTiming)=pqt_time;
static coax_t_PqtStatus(*const typed_sample)(coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,int64_t,coax_v_t_PqtSample)=pqt_sample;
static const uint64_t badbits[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
static double real(uint64_t word){double value;memcpy(&value,&word,8);return value;}
static void read_exact(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}
static void write_exact(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static coax_t_PqtTiming tpoison(void){coax_t_PqtTiming out={1001,1002,1003,1004};return out;}
static coax_t_PqtSample spoison(void){coax_t_PqtSample out;double words[21];for(int i=0;i<21;i++)words[i]=2001+i;memcpy(&out,words,168);return out;}
static coax_t_PqtStatus tcall(int raw,double*x,const int64_t n[5],coax_t_PqtTiming*out){
 coax_v_f64 a={x,n[0]},b={x+7,n[1]},v={x+14,n[2]},c={x+21,n[3]};coax_v_t_PqtTiming result={out,n[4]};
 return raw?pqt_time_raw(a,b,v,c,result):typed_time(a,b,v,c,result);
}
static coax_t_PqtStatus scall(int raw,double*x,const int64_t n[5],int64_t tick,coax_t_PqtSample*out){
 coax_v_f64 a={x,n[0]},b={x+7,n[1]},v={x+14,n[2]},c={x+21,n[3]};coax_v_t_PqtSample result={out,n[4]};
 return raw?pqt_sample_raw(a,b,v,c,tick,result):typed_sample(a,b,v,c,tick,result);
}
struct Tbox{unsigned char prefix[32];coax_t_PqtTiming out[2];unsigned char suffix[32];};
struct Sbox{unsigned char prefix[32];coax_t_PqtSample out[2];unsigned char suffix[32];};
static int zero_edge(const double*x){for(int i=0;i<7;i++)if(x[i]!=x[7+i])return 0;return 1;}
static void check_timing(const double*x,const coax_t_PqtTiming*out){
 NEED(out->steps>=0&&out->steps<=1000000&&out->total_nanoseconds==out->steps*INT64_C(2000000));
 NEED(isfinite(out->duration)&&isfinite(out->required_estimate)&&out->duration==(double)out->steps/500.&&out->required_estimate>=0);
 if(zero_edge(x))NEED(out->steps==0&&out->duration==0&&out->required_estimate==0);
 else NEED(out->steps>=1&&out->duration>out->required_estimate);
}
static coax_t_PqtStatus observed_time(double*x,coax_t_PqtTiming*out){
 struct Tbox box[2],old[2];double input[28];memcpy(input,x,224);coax_t_PqtStatus status[2];int64_t n[]={7,7,7,7,1};
 for(int raw=0;raw<2;raw++){
  memset(&box[raw],0xa5,sizeof box[raw]);box[raw].out[0]=box[raw].out[1]=tpoison();memcpy(&old[raw],&box[raw],sizeof box[raw]);status[raw]=tcall(raw,x,n,box[raw].out);
  NEED(status[raw]==COAX_PqtStatus_ok||status[raw]==COAX_PqtStatus_duration);
  NEED(!memcmp(x,input,224));
  if(status[raw])NEED(!memcmp(&box[raw],&old[raw],sizeof box[raw]));
  else {check_timing(x,&box[raw].out[0]);NEED(!memcmp(box[raw].prefix,old[raw].prefix,32)&&!memcmp(box[raw].suffix,old[raw].suffix,32)&&!memcmp(&box[raw].out[1],&old[raw].out[1],32));}
 }
 NEED(status[0]==status[1]&&!memcmp(&box[0],&box[1],sizeof box[0]));
 if(status[0])memset(out,0,32);else memcpy(out,&box[0].out[0],32);return status[0];
}
static void check_sample(const double*x,int64_t tick,const coax_t_PqtTiming*timing,const coax_t_PqtSample*out){
 double words[21];memcpy(words,out,168);
 for(int i=0;i<7;i++){
  NEED(isfinite(words[i])&&isfinite(words[7+i])&&isfinite(words[14+i]));
  NEED(words[i]>=fmin(x[i],x[7+i])&&words[i]<=fmax(x[i],x[7+i])&&words[i]>=lower[i]&&words[i]<=upper[i]);
  NEED(fabs(words[7+i])<=x[14+i]&&fabs(words[14+i])<=x[21+i]);
 }
 if(tick==0||tick==timing->steps){
  const double*endpoint=tick==timing->steps?x+7:x;NEED(!memcmp(words,endpoint,56));
  for(int i=7;i<21;i++){uint64_t bits;memcpy(&bits,words+i,8);NEED(bits==0);}
 }
}
static coax_t_PqtStatus observed_sample(double*x,int64_t tick,const coax_t_PqtTiming*timing,coax_t_PqtSample*out){
 struct Sbox box[2],old[2];double input[28];memcpy(input,x,224);coax_t_PqtStatus status[2];int64_t n[]={7,7,7,7,1};
 for(int raw=0;raw<2;raw++){
  memset(&box[raw],0xa5,sizeof box[raw]);box[raw].out[0]=box[raw].out[1]=spoison();memcpy(&old[raw],&box[raw],sizeof box[raw]);status[raw]=scall(raw,x,n,tick,box[raw].out);NEED(status[raw]==COAX_PqtStatus_ok||status[raw]==COAX_PqtStatus_duration);NEED(!memcmp(x,input,224));
  if(status[raw])NEED(!memcmp(&box[raw],&old[raw],sizeof box[raw]));
  else {check_sample(x,tick,timing,&box[raw].out[0]);NEED(!memcmp(box[raw].prefix,old[raw].prefix,32)&&!memcmp(box[raw].suffix,old[raw].suffix,32)&&!memcmp(&box[raw].out[1],&old[raw].out[1],168));}
 }
 NEED(status[0]==status[1]&&!memcmp(&box[0],&box[1],sizeof box[0]));
 if(status[0])memset(out,0,168);else memcpy(out,&box[0].out[0],168);return status[0];
}
static void trefused(int raw,double*x,const int64_t n[5],coax_t_PqtStatus expected){
 struct Tbox box,old;memset(&box,0xa5,sizeof box);box.out[0]=box.out[1]=tpoison();memcpy(&old,&box,sizeof box);double input[28];memcpy(input,x,224);NEED(tcall(raw,x,n,box.out)==expected);NEED(!memcmp(&box,&old,sizeof box)&&!memcmp(input,x,224));
}
static void srefused(int raw,double*x,const int64_t n[5],int64_t tick,coax_t_PqtStatus expected){
 struct Sbox box,old;memset(&box,0xa5,sizeof box);box.out[0]=box.out[1]=spoison();memcpy(&old,&box,sizeof box);double input[28];memcpy(input,x,224);NEED(scall(raw,x,n,tick,box.out)==expected);NEED(!memcmp(&box,&old,sizeof box)&&!memcmp(input,x,224));
}
static void refused_both(double*x,int64_t tick,coax_t_PqtStatus expected){int64_t n[]={7,7,7,7,1};for(int raw=0;raw<2;raw++){trefused(raw,x,n,expected);srefused(raw,x,n,tick,expected);}}
static void reset(double*x){const double q[]={-.31,.22,-.14,-1.91,.36,1.47,-.52},d[]={.01,-.02,.03,-.04,.05,-.06,.07};for(int i=0;i<7;i++){x[i]=q[i];x[7+i]=q[i]+d[i];x[14+i]=x[21+i]=1;}}
static void faults(const double*duration_case){
 double x[28];reset(x);x[0]=real(badbits[3]);for(int raw=0;raw<2;raw++)for(int role=0;role<5;role++){
  int64_t n[]={7,7,7,7,1},good=n[role],bad[]={-1,0,good+1,INT64_MIN,INT64_MAX};
  for(int j=0;j<5;j++){n[role]=bad[j];trefused(raw,x,n,COAX_PqtStatus_shape);srefused(raw,x,n,INT64_MIN,COAX_PqtStatus_shape);}
 }
 for(int role=0;role<28;role++)for(int j=0;j<5;j++){reset(x);x[role]=real(badbits[j]);refused_both(x,0,COAX_PqtStatus_number);}
 for(int role=0;role<28;role++)for(int side=0;side<2;side++){
  reset(x);int i=role%7;double endpoint=role<14?(side?upper[i]:lower[i]):role<21?(side?20:.001):(side?100:.001);x[role]=endpoint;
  coax_t_PqtTiming timing;NEED(observed_time(x,&timing)==COAX_PqtStatus_ok);coax_t_PqtSample sample;NEED(observed_sample(x,0,&timing,&sample)==COAX_PqtStatus_ok);NEED(observed_sample(x,timing.steps,&timing,&sample)==COAX_PqtStatus_ok);endpoints++;
  x[role]=nextafter(endpoint,side?INFINITY:-INFINITY);refused_both(x,0,role<14?COAX_PqtStatus_limits:COAX_PqtStatus_range);
 }
 /* Role-first finite/range precedence, including late malformed sample tick. */
 reset(x);x[0]=nextafter(lower[0],-INFINITY);x[7]=real(badbits[3]);refused_both(x,INT64_MIN,COAX_PqtStatus_limits);
 reset(x);x[7]=nextafter(upper[0],INFINITY);x[14]=real(badbits[3]);refused_both(x,INT64_MIN,COAX_PqtStatus_limits);
 reset(x);x[14]=0;x[21]=real(badbits[3]);refused_both(x,INT64_MIN,COAX_PqtStatus_range);
 reset(x);x[0]=real(badbits[3]);x[1]=nextafter(upper[1],INFINITY);refused_both(x,0,COAX_PqtStatus_number);
 reset(x);x[0]=nextafter(lower[0],-INFINITY);x[1]=real(badbits[3]);refused_both(x,0,COAX_PqtStatus_limits);
 reset(x);x[21]=0;refused_both(x,INT64_MIN,COAX_PqtStatus_range);
 /* Numeric equality admits signedzero; q1 representation wins N0/k0. */
 reset(x);memcpy(x+7,x,56);x[0]=0.;x[7]=-0.;coax_t_PqtTiming zero;NEED(observed_time(x,&zero)==COAX_PqtStatus_ok&&zero.steps==0);coax_t_PqtSample sample;NEED(observed_sample(x,0,&zero,&sample)==COAX_PqtStatus_ok);uint64_t bits;memcpy(&bits,&sample.angle.e[0],8);NEED(bits==UINT64_C(0x8000000000000000));
 /* Zeroedge must not bypass velocity/acceleration admission. */
 x[14]=0;refused_both(x,0,COAX_PqtStatus_range);x[14]=1;x[27]=real(badbits[3]);refused_both(x,0,COAX_PqtStatus_number);
 reset(x);coax_t_PqtTiming timing;NEED(observed_time(x,&timing)==COAX_PqtStatus_ok);int64_t n[]={7,7,7,7,1},bad_ticks[]={-1,INT64_MIN,INT64_MAX,timing.steps+1};
 for(int raw=0;raw<2;raw++)for(int i=0;i<4;i++){srefused(raw,x,n,bad_ticks[i],COAX_PqtStatus_range);tick_refusals++;}
 reset(x);memcpy(x+7,x,56);for(int raw=0;raw<2;raw++){srefused(raw,x,n,1,COAX_PqtStatus_range);tick_refusals++;}
 memcpy(x,duration_case,224);for(int raw=0;raw<2;raw++)srefused(raw,x,n,INT64_MIN,COAX_PqtStatus_duration);
}
static void overlaps(void){
 double x[28];reset(x);coax_t_PqtTiming expected;NEED(observed_time(x,&expected)==COAX_PqtStatus_ok);
 /* Every input separately overlaps the actual timing output object. */
 union Tstore{double words[40];struct{double prefix[2];coax_t_PqtTiming out;double suffix[34];}at;}t;
 _Static_assert(sizeof t==320&&offsetof(__typeof__(t.at),out)==16,"timing overlap object");
 for(int role=0;role<4;role++)for(int raw=0;raw<2;raw++)for(int fault=0;fault<2;fault++){
  for(int i=0;i<40;i++)t.words[i]=3001+i;double local[28];memcpy(local,x,224);memcpy(t.words,x+role*7,56);double*views[]={local,local+7,local+14,local+21};views[role]=t.words;
  if(fault)views[role][0]=real(badbits[3]);unsigned char old[320];double inputs[28];memcpy(old,&t,320);for(int j=0;j<4;j++)memcpy(inputs+j*7,views[j],56);
  coax_v_f64 a={views[0],7},b={views[1],7},v={views[2],7},c={views[3],7};coax_v_t_PqtTiming out={&t.at.out,1};coax_t_PqtStatus status=raw?pqt_time_raw(a,b,v,c,out):typed_time(a,b,v,c,out);NEED(status==(fault?COAX_PqtStatus_number:COAX_PqtStatus_ok));
  if(fault)NEED(!memcmp(&t,old,320));else NEED(!memcmp(&t.at.out,&expected,32)&&!memcmp(&t,old,16)&&!memcmp((unsigned char*)&t+48,old+48,272));
  for(int j=0;j<4;j++)if(j!=role)NEED(!memcmp(views[j],inputs+j*7,56));aliases++;
 }
 /* A 168-byte result overlaps q0, q1 and both positive limit arrays. */
 union Sstore{double words[40];struct{double prefix[3];coax_t_PqtSample out;double suffix[16];}at;}s;
 _Static_assert(sizeof s==320&&offsetof(__typeof__(s.at),out)==24,"all-four-input sample overlap");
 double dense[28];reset(dense);for(int i=0;i<7;i++)dense[14+i]=dense[21+i]=1;coax_t_PqtTiming timing;NEED(observed_time(dense,&timing)==COAX_PqtStatus_ok);coax_t_PqtSample wanted;NEED(observed_sample(dense,timing.steps/2,&timing,&wanted)==COAX_PqtStatus_ok);
 for(int raw=0;raw<2;raw++)for(int fault=0;fault<4;fault++){
  for(int i=0;i<40;i++)s.words[i]=4001+i;memcpy(s.words,dense,112);for(int i=14;i<23;i++)s.words[i]=1.;if(fault==1)s.words[0]=real(badbits[3]);unsigned char old[320];memcpy(old,&s,320);int64_t tick=fault==2?timing.steps+1:timing.steps/2;coax_v_f64 a={s.words,7},b={s.words+7,7},v={s.words+14,7},c={s.words+16,7};coax_v_t_PqtSample out={&s.at.out,fault==3?0:1};coax_t_PqtStatus status=raw?pqt_sample_raw(a,b,v,c,tick,out):typed_sample(a,b,v,c,tick,out);NEED(status==(fault==1?COAX_PqtStatus_number:fault==2?COAX_PqtStatus_range:fault==3?COAX_PqtStatus_shape:COAX_PqtStatus_ok));
  if(fault)NEED(!memcmp(&s,old,320));else NEED(!memcmp(&s.at.out,&wanted,168)&&!memcmp(&s,old,24)&&!memcmp((unsigned char*)&s+192,old+192,128));aliases++;
 }
}
int main(int argc,char**argv){
 NEED(argc==4);uint16_t endian=1;NEED(*(uint8_t*)&endian==1);uint64_t fpcr=UINT64_MAX;
#if defined(__aarch64__)
 __asm__ volatile("mrs %0,fpcr":"=r"(fpcr));
#endif
 NEED(fpcr==0);FILE*f=fopen(argv[1],"rb");NEED(f);uint32_t header[4];read_exact(f,header,16);NEED(header[0]==UINT32_C(0x31545150)&&header[1]==27&&header[2]>0&&header[2]<=1024&&header[3]==1);double edges[27][28];read_exact(f,edges,sizeof edges);int64_t queries[1024][2];read_exact(f,queries,header[2]*16);read_exact(f,lower,56);read_exact(f,upper,56);NEED(fgetc(f)==EOF&&!fclose(f));
 for(int i=0;i<27;i++)for(int j=0;j<28;j++)NEED(isfinite(edges[i][j]));for(uint32_t i=0;i<header[2];i++)NEED(queries[i][0]>=0&&queries[i][0]<27&&queries[i][1]>=0&&queries[i][1]<=1000000);for(int i=0;i<7;i++)NEED(isfinite(lower[i])&&isfinite(upper[i])&&lower[i]<upper[i]);
 FILE*tf=fopen(argv[2],"wbx"),*sf=fopen(argv[3],"wbx");NEED(tf&&sf);coax_t_PqtTiming timings[27];int cap_index=-1;
 for(int i=0;i<27;i++){int64_t status=observed_time(edges[i],&timings[i]);write_exact(tf,&status,8);write_exact(tf,&timings[i],32);if(status==COAX_PqtStatus_duration){cap_index=i;duration_refusals++;}}
 NEED(cap_index>=0);for(uint32_t i=0;i<header[2];i++){int e=(int)queries[i][0];coax_t_PqtSample sample;int64_t status=observed_sample(edges[e],queries[i][1],&timings[e],&sample);write_exact(sf,&status,8);write_exact(sf,&sample,168);sample_count++;}
 NEED(!fclose(tf)&&!fclose(sf));faults(edges[cap_index]);overlaps();NEED(aliases==24&&endpoints==56&&tick_refusals==10);
 printf("{\"passed\":true,\"SDK_calls\":0,\"edges\":27,\"samples\":%zu,\"time_record_bytes\":40,\"sample_record_bytes\":176,\"assertions\":%zu,\"alias_cases\":%zu,\"source_and_limit_endpoints\":%zu,\"tick_refusals\":%zu,\"duration_refused_edges\":%zu,\"nonfinite_role_patterns\":140,\"fpcr\":%llu}\n",sample_count,assertions,aliases,endpoints,tick_refusals,duration_refusals,(unsigned long long)fpcr);return 0;
}
