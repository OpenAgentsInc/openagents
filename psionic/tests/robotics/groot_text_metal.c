/* Effects-only original text-prefix runner. Coax owns tensor bindings, complete
   micro-op order, request positions, flags, completion and copy admission. */
#include "text-plan.h"
#include "groot_gpu_dispatch.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#define TRY(e) do{PmStatus st_=(e);if(st_){fprintf(stderr,"%s: %d %s\n",#e,st_,pm_log());exit(1);}}while(0)
static void need(int b,const char *why){if(!b){fprintf(stderr,"%s\n",why);exit(1);}}
static void path(char *out,size_t cap,const char *root,const char *name){int n=snprintf(out,cap,"%s/%s",root,name);need(n>0&&(size_t)n<cap,"path too long");}
static unsigned char *read_file(const char *name,size_t *n){FILE *f=fopen(name,"rb");need(f!=NULL,"input file missing");need(!fseek(f,0,SEEK_END),"input seek");long z=ftell(f);need(z>=0,"input size");rewind(f);unsigned char *p=malloc((size_t)z+1);need(p!=NULL,"allocation failed");need(fread(p,1,(size_t)z,f)==(size_t)z,"short input read");need(!fclose(f),"input close");p[z]=0;*n=(size_t)z;return p;}
static PmModule compile(const char *filename){size_t n;unsigned char *s=read_file(filename,&n);PmModule m;double sec;TRY(pm_compile((char*)s,n,filename,&m,&sec));free(s);printf("compiled %s in %.6f s\n",filename,sec);return m;}
int main(int argc,char **argv){
 need(argc==9,"usage: text-metal FULL_SHARD_DIRECTORY INPUT_DIRECTORY TILE.metal DEVICE.metal TEXT.metal NEW_OUTPUT_DIRECTORY DEVICE.txt WIDE.metal");
 char filename[4096];path(filename,sizeof filename,argv[2],"ids.i64");size_t ids_bytes;unsigned char *id_bytes=read_file(filename,&ids_bytes);need(ids_bytes%8==0,"unaligned token bytes");int64_t tokens=(int64_t)(ids_bytes/8);need(glp_tokens(tokens),"Coax token extent refused");
 PmDeviceInfo device;TRY(pm_open(&device));printf("text Metal device: %s\n",device.name);
 const char *files[]={"model-00001-of-00002.safetensors","model-00002-of-00002.safetensors"};unsigned char *maps[2];int64_t sizes[2];coax_t_P5Tensor *inventory=calloc(4096,sizeof *inventory),*draft=calloc(4096,sizeof *draft);need(inventory&&draft,"inventory allocation");int64_t tensors=0;
 for(int i=0;i<2;i++){path(filename,sizeof filename,argv[1],files[i]);int fd=open(filename,O_RDONLY);need(fd>=0,"shard open");struct stat stat;need(!fstat(fd,&stat),"shard stat");sizes[i]=stat.st_size;maps[i]=mmap(NULL,(size_t)sizes[i],PROT_READ,MAP_PRIVATE,fd,0);close(fd);need(maps[i]!=MAP_FAILED,"shard mmap");uint64_t header;memcpy(&header,maps[i],8);need(header<=1048576&&header<=(uint64_t)sizes[i]-8,"bounded shard header");coax_t_PwResult r=gca_header((coax_string){(char*)maps[i]+8,(int64_t)header},sizes[i],i,(coax_v_t_P5Tensor){inventory+tensors,4096-tensors},(coax_v_t_P5Tensor){draft,4096},0);need(r.status==COAX_PwStatus_ok,"Coax original header refused");tensors+=r.written;}
 int buffer_count=(int)glp_buffer_count(),kernel_count=(int)glp_kernel_count();PmBuffer *buffers=calloc((size_t)buffer_count,sizeof *buffers);PmKernel *kernels=calloc((size_t)kernel_count,sizeof *kernels);need(buffers&&kernels,"GPU handle allocation");
 for(int b=0;b<buffer_count;b++)TRY(pm_alloc((size_t)glp_buffer_bytes(b,tokens),&buffers[b]));
 for(int p=0;p<glp_weight_count();p++){coax_t_GvpBind r=glp_bind(p,(coax_v_t_P5Tensor){inventory,tensors},(coax_v_s64){sizes,2});need(r.status==COAX_GvpStatus_ok,"Coax learned tensor binding refused");unsigned char *source=maps[r.shard]+r.begin;if(r.decode){coax_t_QmResult decoded=gl_bias(COAX_QmDtype_bf16,r.count,(coax_v_u8){source,r.bytes},(coax_v_f32){pm_contents(buffers[p]),r.count},0);need(!decoded.status,"Coax BF16 decode refused");}else memcpy(pm_contents(buffers[p]),source,(size_t)r.bytes);}
 memcpy(pm_contents(buffers[194]),id_bytes,ids_bytes);free(id_bytes);
 int64_t *request=calloc((size_t)tokens*6,sizeof(*request)),*map_work=calloc((size_t)tokens,sizeof(*map_work));float *rope_work=calloc((size_t)tokens*128,sizeof(*rope_work));need(request&&map_work&&rope_work,"request workspace allocation");
 coax_t_GrRead request_read=gr_plan((coax_v_s64){pm_contents(buffers[194]),tokens},0,(coax_v_s64){request,tokens*6},0);need(!request_read.status&&request_read.count==tokens,"Coax original request grammar refused");
 coax_t_GtRead mapping=gt_images((coax_v_s64){pm_contents(buffers[194]),tokens},(coax_v_s64){pm_contents(buffers[195]),tokens},(coax_v_s64){map_work,tokens},0);need(!mapping.status,"Coax image ordinals refused");
 coax_t_GtRead rotary=gt_rope((coax_v_s64){request+tokens*3,tokens*3},(coax_v_f32){pm_contents(buffers[196]),tokens*128},(coax_v_f32){rope_work,tokens*128},0);need(!rotary.status,"Coax text rotary table refused");free(request);free(map_work);free(rope_work);
 for(int input=0;input<4;input++){coax_t_GvpCapture c=glp_input(input);need(c.ok&&c.prefix.count>0&&c.prefix.count<128,"Coax input descriptor refused");char name[128];memcpy(name,c.prefix.data,(size_t)c.prefix.count);name[c.prefix.count]=0;path(filename,sizeof filename,argv[2],name);size_t n;unsigned char *raw=read_file(filename,&n);need(n==(size_t)glp_buffer_bytes(c.buffer,tokens),"input byte extent");memcpy(pm_contents(buffers[c.buffer]),raw,n);free(raw);}
 PmModule modules[4]={compile(argv[3]),compile(argv[4]),compile(argv[5]),compile(argv[8])};for(int k=0;k<kernel_count;k++){coax_t_GvpKernel s=glp_kernel(k);need(s.library>=0&&s.library<4&&s.name.count>0&&s.name.count<128,"Coax kernel descriptor invalid");char name[128];memcpy(name,s.name.data,(size_t)s.name.count);name[s.name.count]=0;TRY(pm_kernel(modules[s.library],name,&kernels[k]));}
 coax_t_GvpCursor cursor={0};coax_v_t_GvpCursor state={&cursor,1};double total_gpu=0;int launches=0;
 for(int stage=0;stage<glp_stage_count();stage++){
  coax_t_GvpStage s=glp_stage(stage,tokens);need(!s.status,"Coax stage refused");memset(pm_contents(buffers[192]),9,(size_t)s.flags);PmBatch batch;TRY(pm_begin(&batch));
  for(int n=0;n<s.nodes;n++){coax_t_GvpNode node=glp_node(stage,n,tokens);PmStatus effect=groot_gpu_dispatch(batch,node,kernels,(size_t)kernel_count,buffers,(size_t)buffer_count);coax_t_GvpStatus accepted=glp_sent(state,stage,n,tokens,effect);if(accepted){(void)pm_wait(batch,NULL);fprintf(stderr,"node receipt stage%d node%d refused:%u effect%d %s\n",stage,n,accepted,effect,pm_log());return 1;}launches++;}
  double seconds;PmStatus effect=pm_wait(batch,&seconds);coax_t_GvpStatus accepted=glp_done(state,stage,tokens,effect,(coax_v_u8){pm_contents(buffers[192]),s.flags});if(accepted){fprintf(stderr,"stage%d completion refused:%u effect%d %s\n",stage,accepted,effect,pm_log());return 1;}total_gpu+=seconds;
  coax_t_GvpCapture c=glp_capture(stage,tokens);need(c.ok,"Coax capture descriptor refused");float *output=malloc((size_t)c.count*4);need(output!=NULL,"capture allocation");coax_t_GvpRead r=glp_copy((coax_v_u32){pm_contents(buffers[c.buffer]),c.count},(coax_v_f32){output,c.count},0);need(!r.status&&r.written==c.count,"Coax private capture publication refused");char name[128];int n=c.number<0?snprintf(name,sizeof name,"%.*s.f32",(int)c.prefix.count,c.prefix.data):snprintf(name,sizeof name,"%.*s%lld.f32",(int)c.prefix.count,c.prefix.data,(long long)c.number);need(n>0&&(size_t)n<sizeof name,"capture name");path(filename,sizeof filename,argv[6],name);FILE *f=fopen(filename,"wbx");need(f!=NULL,"new capture file unavailable");need(fwrite(output,4,(size_t)c.count,f)==(size_t)c.count&&!fclose(f),"capture write");free(output);
  printf("text stage%d: %lld nodes, %lld flags passed, GPU %.6f s\n",stage,(long long)s.nodes,(long long)s.flags,seconds);fflush(stdout);
 }
 need(glp_complete(cursor),"Coax text prefix not complete");FILE *f=fopen(argv[7],"wx");need(f!=NULL,"new device file unavailable");need(fprintf(f,"%s\n%d\n%.12g\n%d\n",device.name,device.apple_family,total_gpu,launches)>0&&!fclose(f),"device record");
 for(int i=0;i<kernel_count;i++)TRY(pm_kernel_free(kernels[i]));for(int i=0;i<4;i++)TRY(pm_unload(modules[i]));for(int i=0;i<buffer_count;i++)TRY(pm_free(buffers[i]));for(int i=0;i<2;i++)munmap(maps[i],(size_t)sizes[i]);free(inventory);free(draft);free(buffers);free(kernels);return 0;
}
