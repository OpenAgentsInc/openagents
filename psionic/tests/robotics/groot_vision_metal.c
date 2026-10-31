/* Effects-only original Qwen3-VL runner. Every weight lookup, shape, ordered
   micro-op, flag extent and capture name comes from the Coax graph. */
#include "vision-plan.h"
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
 need(argc==9,"usage: vision-metal FULL_SHARD_DIRECTORY PIXELS.f32 TILE.metal DEVICE.metal VISION.metal WIDE.metal NEW_OUTPUT_DIRECTORY DEVICE.txt");
 PmDeviceInfo device;TRY(pm_open(&device));printf("vision Metal device: %s\n",device.name);
 const char *files[]={"model-00001-of-00002.safetensors","model-00002-of-00002.safetensors"};unsigned char *maps[2];int64_t sizes[2];coax_t_P5Tensor *inventory=calloc(4096,sizeof *inventory),*draft=calloc(4096,sizeof *draft);need(inventory&&draft,"inventory allocation");int64_t tensors=0;
 for(int i=0;i<2;i++){char filename[4096];path(filename,sizeof filename,argv[1],files[i]);int fd=open(filename,O_RDONLY);need(fd>=0,"shard open");struct stat stat;need(!fstat(fd,&stat),"shard stat");sizes[i]=stat.st_size;maps[i]=mmap(NULL,(size_t)sizes[i],PROT_READ,MAP_PRIVATE,fd,0);close(fd);need(maps[i]!=MAP_FAILED,"shard mmap");uint64_t header;memcpy(&header,maps[i],8);need(header<=1048576&&header<=(uint64_t)sizes[i]-8,"bounded shard header");coax_t_PwResult r=gca_header((coax_string){(char*)maps[i]+8,(int64_t)header},sizes[i],i,(coax_v_t_P5Tensor){inventory+tensors,4096-tensors},(coax_v_t_P5Tensor){draft,4096},0);need(r.status==COAX_PwStatus_ok,"Coax source header refused");tensors+=r.written;}
 int buffers_count=(int)gvp_buffer_count(),kernel_count=(int)gvp_kernel_count();PmBuffer *buffers=calloc((size_t)buffers_count,sizeof *buffers);PmKernel *kernels=calloc((size_t)kernel_count,sizeof *kernels);need(buffers&&kernels,"GPU handle allocation");
 for(int b=0;b<buffers_count;b++)TRY(pm_alloc((size_t)gvp_buffer_bytes(b),&buffers[b]));
 for(int p=0;p<gvp_weight_count();p++){coax_t_GvpBind r=gvp_bind(p,(coax_v_t_P5Tensor){inventory,tensors},(coax_v_s64){sizes,2});need(r.status==COAX_GvpStatus_ok,"Coax original learned tensor binding refused");unsigned char *source=maps[r.shard]+r.begin;if(r.decode){coax_t_QmResult decoded=gl_bias(COAX_QmDtype_bf16,r.count,(coax_v_u8){source,r.bytes},(coax_v_f32){pm_contents(buffers[p]),r.count},0);need(!decoded.status,"Coax BF16 parameter decode refused");}else memcpy(pm_contents(buffers[p]),source,(size_t)r.bytes);}
 need(gvp_prepare((coax_v_s64){pm_contents(buffers[327]),512},(coax_v_u32){pm_contents(buffers[335]),1024})==COAX_GvpStatus_ok,"Coax camera/zero preparation refused");size_t pixels_size;unsigned char *pixels=read_file(argv[2],&pixels_size);need(pixels_size==(size_t)gvp_buffer_bytes(330),"packed pixel bytes");memcpy(pm_contents(buffers[330]),pixels,pixels_size);free(pixels);
 PmModule modules[4]={compile(argv[3]),compile(argv[4]),compile(argv[5]),compile(argv[6])};for(int k=0;k<kernel_count;k++){coax_t_GvpKernel s=gvp_kernel(k);need(s.library>=0&&s.library<4&&s.name.count>0&&s.name.count<128,"Coax kernel descriptor invalid");char name[128];memcpy(name,s.name.data,(size_t)s.name.count);name[s.name.count]=0;TRY(pm_kernel(modules[s.library],name,&kernels[k]));}
 coax_t_GvpCursor cursor={0};coax_v_t_GvpCursor state={&cursor,1};double total_gpu=0;
 for(int sequence=0;sequence<gvp_stage_count();sequence++){
  coax_t_GvpStage s=gvp_stage(sequence);need(!s.status,"Coax stage refused");memset(pm_contents(buffers[329]),9,(size_t)s.flags);PmBatch batch;TRY(pm_begin(&batch));
  for(int n=0;n<s.nodes;n++){coax_t_GvpNode node=gvp_node(sequence,n);PmStatus effect=groot_gpu_dispatch(batch,node,kernels,(size_t)kernel_count,buffers,(size_t)buffers_count);coax_t_GvpStatus accepted=gvp_sent(state,sequence,n,effect);if(accepted){(void)pm_wait(batch,NULL);fprintf(stderr,"node receipt stage%d node%d refused:%u effect%d %s\n",sequence,n,accepted,effect,pm_log());return 1;}}
  double seconds;PmStatus effect=pm_wait(batch,&seconds);coax_t_GvpStatus accepted=gvp_done(state,sequence,effect,(coax_v_u8){pm_contents(buffers[329]),s.flags});if(accepted){fprintf(stderr,"stage%d completion refused:%u effect%d %s\n",sequence,accepted,effect,pm_log());return 1;}total_gpu+=seconds;
  for(int slot=0;slot<2;slot++){coax_t_GvpCapture c=gvp_capture(sequence,slot);if(!c.ok)continue;float *output=malloc((size_t)c.count*4);need(output!=NULL,"capture allocation");coax_t_GvpRead r=gvp_copy((coax_v_u32){pm_contents(buffers[c.buffer]),c.count},(coax_v_f32){output,c.count},0);need(!r.status&&r.written==c.count,"Coax private capture publication refused");char name[256],filename[4096];int n=c.number<0?snprintf(name,sizeof name,"%.*s%.*s.f32",(int)c.prefix.count,c.prefix.data,(int)c.suffix.count,c.suffix.data):snprintf(name,sizeof name,"%.*s%lld%.*s.f32",(int)c.prefix.count,c.prefix.data,(long long)c.number,(int)c.suffix.count,c.suffix.data);need(n>0&&(size_t)n<sizeof name,"capture name");path(filename,sizeof filename,argv[7],name);FILE *f=fopen(filename,"wbx");need(f!=NULL,"new capture file unavailable");need(fwrite(output,4,(size_t)c.count,f)==(size_t)c.count&&!fclose(f),"capture write");free(output);}
  printf("vision stage%d kind%lld layer%lld merger%lld: %lld nodes, %lld flags passed, GPU %.6f s\n",sequence,(long long)s.kind,(long long)s.layer,(long long)s.merger,(long long)s.nodes,(long long)s.flags,seconds);fflush(stdout);
 }
 need(gvp_complete(cursor),"Coax full vision graph not complete");FILE *f=fopen(argv[8],"wx");need(f!=NULL,"new device file unavailable");need(fprintf(f,"%s\n%d\n%.12g\n",device.name,device.apple_family,total_gpu)>0&&!fclose(f),"device record");
 for(int i=0;i<kernel_count;i++)TRY(pm_kernel_free(kernels[i]));for(int i=0;i<4;i++)TRY(pm_unload(modules[i]));for(int i=0;i<buffers_count;i++)TRY(pm_free(buffers[i]));for(int i=0;i<2;i++)munmap(maps[i],(size_t)sizes[i]);free(inventory);free(draft);free(buffers);free(kernels);return 0;
}
