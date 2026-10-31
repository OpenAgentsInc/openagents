/* Original category-2 GR00T state encoder on Metal. Platform effects only:
   Coax chooses every shape, launch argument, order and publication decision.
   CPU gl_pack/gl_bias decode data; no CPU projection is called or substituted. */
#include "plan.h"
#include "metal-host.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define TRY(e) do{PmStatus s_=(e);if(s_!=PM_OK){fprintf(stderr,"%s: %d: %s\n",#e,s_,pm_log());exit(1);}}while(0)
static void require(int b,const char *why){if(!b){fprintf(stderr,"%s\n",why);exit(1);}}
static unsigned char *read_all(const char *path,size_t *n){FILE *f=fopen(path,"rb");require(f!=NULL,"input file unavailable");require(fseek(f,0,SEEK_END)==0,"input seek failed");long z=ftell(f);require(z>=0,"input size failed");rewind(f);unsigned char *p=malloc((size_t)z+1);require(p!=NULL,"input allocation failed");require(fread(p,1,(size_t)z,f)==(size_t)z,"short input read");require(fclose(f)==0,"input close failed");p[z]=0;*n=(size_t)z;return p;}
static PmModule module(const char *path,const char *name){size_t n;unsigned char *p=read_all(path,&n);PmModule m;double sec;TRY(pm_compile((char*)p,n,name,&m,&sec));free(p);printf("compiled %s: %.6f s\n",name,sec);return m;}
static void part(const char *root,const char *name,int inputs,int outputs,PmBuffer destination,int bias){char path[4096];int n=snprintf(path,sizeof path,"%s/%s.bf16",root,name);require(n>0&&(size_t)n<sizeof path,"part path too long");size_t bytes;unsigned char *raw=read_all(path,&bytes);coax_t_QmResult r=bias?gl_bias(COAX_QmDtype_bf16,outputs,(coax_v_u8){raw,(int64_t)bytes},(coax_v_f32){pm_contents(destination),outputs},0):gl_pack(COAX_QmDtype_bf16,inputs,outputs,(coax_v_u8){raw,(int64_t)bytes},(coax_v_u8){pm_contents(destination),(int64_t)bytes},0);free(raw);require(r.status==COAX_QmStatus_ok,"Coax original part packing refused");}
int main(int argc,char **argv){
 require(argc==6,"usage: groot-metal CACHE TILE.metal BIAS.metal OUTPUT.f32 DEVICE.txt");
 /* Open is required. An absent/incompatible GPU is a failure, never SKIP. */
 PmDeviceInfo info;TRY(pm_open(&info));printf("Metal device: %s; Apple family %d\n",info.name,info.apple_family);
 PmModule tile=module(argv[2],"groot-state-tile"),bias=module(argv[3],"groot-state-bias");PmKernel kernels[3];TRY(pm_kernel(tile,"dt_project_16x64",&kernels[0]));TRY(pm_kernel(bias,"gms_bias_relu",&kernels[1]));TRY(pm_kernel(bias,"gms_bias",&kernels[2]));
 coax_t_GmsLayout layout=gms_layout();PmBuffer buffers[8];int64_t capacities[8];
 for(int i=0;i<8;i++){capacities[i]=layout.bytes.e[i];TRY(pm_alloc((size_t)capacities[i],&buffers[i]));require(pm_contents(buffers[i])!=NULL,"Metal shared storage unavailable");}
 part(argv[1],"state-w1",132,1024,buffers[0],0);part(argv[1],"state-w2",1024,1536,buffers[1],0);part(argv[1],"state-b1",0,1024,buffers[2],1);part(argv[1],"state-b2",0,1536,buffers[3],1);
 /* Hold both observations privately; create the public file only after every
    case passes the Coax command/numeric/publication gate. */
 float outputs[5120];double times[2];
 for(int case_id=0;case_id<2;case_id++){
  float *x=pm_contents(buffers[4]);for(int i=0;i<132;i++)x[i]=case_id?(float)(i%17-8)/16.0f:i<8?(float)(i-3)/4.0f:0.0f;
  memset(pm_contents(buffers[5]),0xff,(size_t)capacities[5]);memset(pm_contents(buffers[6]),0xff,(size_t)capacities[6]);memset(pm_contents(buffers[7]),0xff,(size_t)capacities[7]);
  coax_t_GmsCursor cursor=gms_begin((coax_v_s64){capacities,8},0);coax_v_t_GmsCursor state={&cursor,1};require(cursor.status==COAX_GmsStatus_ok,"Coax admission refused");PmBatch batch;TRY(pm_begin(&batch));
  for(int node=0;node<layout.nodes;node++){
   coax_t_GmsNode n=gms_node(node);require(n.status==0,"Coax launch node refused");PmArg args[6]={0};
   for(int k=0;k<n.argc;k++){coax_t_GmsArg a=n.args.e[k];args[k].kind=(PmArgKind)a.kind;if(a.kind==PM_ARG_INT)args[k].i=a.integer;else{require(a.kind==PM_ARG_VIEW&&a.buffer>=0&&a.buffer<8,"Coax argument metadata malformed");args[k].buffer=buffers[a.buffer];args[k].count=a.count;}}
   PmStatus status=pm_launch(batch,kernels[n.kernel],n.items,(int)n.lanes,(int)n.block,args,(int)n.argc);coax_t_GmsStatus accepted=gms_sent(state,node,status);if(accepted!=COAX_GmsStatus_ok){/* Close even a failed batch; publish remains denied. */(void)pm_wait(batch,NULL);fprintf(stderr,"Coax launch receipt refused: %u / platform %d: %s\n",(unsigned)accepted,status,pm_log());return 1;}
  }
  PmStatus waited=pm_wait(batch,&times[case_id]);require(gms_waited(state,waited)==COAX_GmsStatus_ok,"Coax wait receipt refused");
  coax_t_GmsRead publication=gms_publish(state,(coax_v_u8){pm_contents(buffers[7]),2560},(coax_v_u32){pm_contents(buffers[5]),1024},(coax_v_u32){pm_contents(buffers[6]),1536},(coax_v_f32){outputs+case_id*2560,2560},0);require(publication.status==COAX_GmsStatus_ok&&publication.written==2560,"Coax original GPU publication refused");printf("case %d: 4 Coax launches, pm_wait=%d, %lld values published; GPU %.9f s\n",case_id,waited,(long long)publication.written,times[case_id]);
 }
 FILE *f=fopen(argv[4],"wbx");require(f!=NULL,"output file already exists/unavailable");require(fwrite(outputs,sizeof(float),5120,f)==5120,"short output write");require(fclose(f)==0,"output close failed");
 f=fopen(argv[5],"wx");require(f!=NULL,"device record already exists/unavailable");require(fprintf(f,"%s\n%d\n%.12g\n%.12g\n",info.name,info.apple_family,times[0],times[1])>0,"device record failed");require(fclose(f)==0,"device close failed");
 for(int i=0;i<8;i++)TRY(pm_free(buffers[i]));for(int i=0;i<3;i++)TRY(pm_kernel_free(kernels[i]));TRY(pm_unload(tile));TRY(pm_unload(bias));return 0;
}
