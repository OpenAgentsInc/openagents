/* Effects-only full category2 action-head exercise. Coax owns every tensor
   name, shape, transform, micro-op, flag boundary, receipt and capture. */
#include "action-plan.h"
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
static void need(int b,const char*why){if(!b){fprintf(stderr,"%s\n",why);exit(1);}}
static void path(char*out,size_t cap,const char*root,const char*name){int n=snprintf(out,cap,"%s/%s",root,name);need(n>0&&(size_t)n<cap,"path too long");}
static unsigned char*read_file(const char*name,size_t*n){FILE*f=fopen(name,"rb");need(f!=NULL,"input missing");need(!fseek(f,0,SEEK_END),"input seek");long z=ftell(f);need(z>=0,"input size");rewind(f);unsigned char*p=malloc((size_t)z+1);need(p!=NULL,"allocation");need(fread(p,1,(size_t)z,f)==(size_t)z,"input short read");need(!fclose(f),"input close");p[z]=0;*n=(size_t)z;return p;}
static void write_file(const char*root,const char*name,const void*data,size_t bytes){char filename[4096];path(filename,sizeof filename,root,name);FILE*f=fopen(filename,"wbx");need(f!=NULL,"new output required");need(fwrite(data,1,bytes,f)==bytes&&!fclose(f),"output write");}
static PmModule compile(const char*filename){size_t n;unsigned char*s=read_file(filename,&n);PmModule m;double seconds;TRY(pm_compile((char*)s,n,filename,&m,&seconds));free(s);printf("action module %s compiled %.6f s\n",filename,seconds);return m;}
int main(int argc,char**argv){
 need(argc==8,"usage: action-metal FULL_SHARDS INPUT_DIRECTORY TILE.metal DEVICE.metal ACTION.metal NEW_OUTPUT_DIRECTORY DEVICE.txt");PmDeviceInfo device;TRY(pm_open(&device));printf("action Metal device: %s\n",device.name);fflush(stdout);
 const char*files[]={"model-00001-of-00002.safetensors","model-00002-of-00002.safetensors"};unsigned char*maps[2];int64_t sizes[2];coax_t_P5Tensor*inventory=calloc(4096,sizeof*inventory),*draft=calloc(4096,sizeof*draft);need(inventory&&draft,"inventory allocation");int64_t tensors=0;
 for(int i=0;i<2;i++){char filename[4096];path(filename,sizeof filename,argv[1],files[i]);int fd=open(filename,O_RDONLY);need(fd>=0,"shard open");struct stat stat;need(!fstat(fd,&stat),"shard stat");sizes[i]=stat.st_size;maps[i]=mmap(NULL,(size_t)sizes[i],PROT_READ,MAP_PRIVATE,fd,0);close(fd);need(maps[i]!=MAP_FAILED,"shard mmap");uint64_t header;memcpy(&header,maps[i],8);need(header<=1048576&&header<=(uint64_t)sizes[i]-8,"bounded header");coax_t_PwResult r=gca_header((coax_string){(char*)maps[i]+8,(int64_t)header},sizes[i],i,(coax_v_t_P5Tensor){inventory+tensors,4096-tensors},(coax_v_t_P5Tensor){draft,4096},0);need(!r.status,"Coax header refused");tensors+=r.written;}
 int nb=(int)gac_buffer_count(),nk=(int)gac_kernel_count();PmBuffer*buffers=calloc(nb,sizeof*buffers);PmKernel*kernels=calloc(nk,sizeof*kernels);need(buffers&&kernels,"handles allocation");for(int b=0;b<nb;b++)TRY(pm_alloc((size_t)gac_buffer_bytes(b),buffers+b));
 for(int p=0;p<gac_weight_count();p++){coax_t_GacBinding r=gac_bind(p,(coax_v_t_P5Tensor){inventory,tensors},(coax_v_s64){sizes,2});if(r.status){fprintf(stderr,"Coax weight%d binding refused:%u\n",p,r.status);return 1;}unsigned char*source=maps[r.shard]+r.begin;void*destination=pm_contents(buffers[p]);
  if(r.transpose){coax_t_QmResult q=gl_pack(COAX_QmDtype_bf16,r.cols,r.rows,(coax_v_u8){source,r.bytes},(coax_v_u8){destination,r.target},0);need(!q.status,"Coax category transpose refused");}
  else if(r.decode){coax_t_GvpRead q=gac_decode(p,(coax_v_u8){source,r.bytes},(coax_v_f32){destination,r.target/4},0);need(!q.status,"Coax parameter decode refused");}
  else memcpy(destination,source,(size_t)r.bytes);
 }printf("all537 original action tensors bound and prepared\n");fflush(stdout);
 const char*inputs[]={"backbone.f32","state.f32","noise.f32"};int input_buffers[]={537,538,539};for(int i=0;i<3;i++){char filename[4096];path(filename,sizeof filename,argv[2],inputs[i]);size_t n;unsigned char*data=read_file(filename,&n);need(n==(size_t)gac_buffer_bytes(input_buffers[i]),"input byte extent");memcpy(pm_contents(buffers[input_buffers[i]]),data,n);free(data);}char filename[4096];path(filename,sizeof filename,argv[2],"ids.i64");size_t n;unsigned char*ids=read_file(filename,&n);need(n==1120,"ids extent");need(!gac_masks((coax_v_s64){(int64_t*)ids,140},(coax_v_u8){pm_contents(buffers[540]),140},(coax_v_u8){pm_contents(buffers[541]),140},0),"Coax action masks refused");free(ids);
 char wide_path[4096];path(wide_path,sizeof wide_path,argv[2],"wide.metal");PmModule modules[4]={compile(argv[3]),compile(argv[4]),compile(argv[5]),compile(wide_path)};for(int k=0;k<nk;k++){coax_t_GvpKernel s=gac_kernel(k);need(s.library>=0&&s.library<4&&s.name.count>0&&s.name.count<128,"kernel descriptor");char name[128];memcpy(name,s.name.data,(size_t)s.name.count);name[s.name.count]=0;TRY(pm_kernel(modules[s.library],name,kernels+k));}
 coax_t_GvpCursor cursor={0};coax_v_t_GvpCursor state={&cursor,1};double total_gpu=0;
 for(int sequence=0;sequence<gac_stage_count();sequence++){coax_t_GacStage s=gac_stage(sequence);need(!s.status,"Coax stage refused");memset(pm_contents(buffers[567]),9,(size_t)s.flags);PmBatch batch;TRY(pm_begin(&batch));
  for(int at=0;at<s.nodes;at++){coax_t_GvpNode node=gac_node(sequence,at);PmStatus effect=groot_gpu_dispatch(batch,node,kernels,nk,buffers,nb);coax_t_GvpStatus accepted=gac_sent(state,sequence,at,effect);if(accepted){(void)pm_wait(batch,NULL);fprintf(stderr,"action node stage%d node%d refused:%u effect%d %s\n",sequence,at,accepted,effect,pm_log());return 1;}}
  double seconds;PmStatus effect=pm_wait(batch,&seconds);coax_t_GvpStatus accepted=gac_done(state,sequence,effect,(coax_v_u8){pm_contents(buffers[567]),s.flags});if(accepted){fprintf(stderr,"action stage%d completion refused:%u effect%d %s\n",sequence,accepted,effect,pm_log());return 1;}total_gpu+=seconds;
  for(int slot=0;slot<3;slot++){coax_t_GacCapture c=gac_capture(sequence,slot);if(!c.ok)continue;char name[256];need(c.name.count>0&&c.name.count<240,"capture name");snprintf(name,sizeof name,"%.*s.f32",(int)c.name.count,c.name.data);write_file(argv[6],name,(unsigned char*)pm_contents(buffers[c.buffer])+c.offset,(size_t)c.count*4);}
  printf("action stage%d kind%lld step%lld layer%lld: %lld nodes, %lld flags, GPU%.6f s\n",sequence,(long long)s.kind,(long long)s.step,(long long)s.layer,(long long)s.nodes,(long long)s.flags,seconds);fflush(stdout);
 }
 float published[5280];coax_t_GvpRead r=gac_publish(state,(coax_v_u32){pm_contents(buffers[539]),5280},(coax_v_f32){published,5280},0);need(!r.status&&r.written==5280,"Coax final publication refused");write_file(argv[6],"published-actions.f32",published,sizeof published);FILE*f=fopen(argv[7],"wx");need(f!=NULL,"new device record");need(fprintf(f,"%s\n%d\n%.12g\n",device.name,device.apple_family,total_gpu)>0&&!fclose(f),"device write");
 for(int k=0;k<nk;k++)TRY(pm_kernel_free(kernels[k]));for(int i=0;i<4;i++)TRY(pm_unload(modules[i]));for(int b=0;b<nb;b++)TRY(pm_free(buffers[b]));for(int i=0;i<2;i++)munmap(maps[i],sizes[i]);free(inventory);free(draft);free(buffers);free(kernels);return 0;
}
