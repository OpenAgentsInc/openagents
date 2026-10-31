/* Firm reason5: effects-only resident host. Generated Coax chooses every
   arithmetic operation, graph, source tensor, extent, transfer and receipt. */
#include "panda-policy-generated.h"
#include "panda-policy.h"
#include "groot_gpu_dispatch.h"
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
typedef struct {coax_t_PvnStep step;coax_t_GvpNode *nodes;} PolicyStage;
struct PandaPolicy {
 char error[512];char *capture_directory;int ready,complete;int64_t tokens;PmBuffer *buffers;PmKernel *kernels;PmModule modules[6];int modules_count,kernels_count,buffers_count;
 PolicyStage *stages;int stages_count;
 unsigned char *maps[2],*pst;int64_t sizes[2];size_t pst_bytes;
 coax_t_P5Tensor *inventory;int64_t tensors;
 uint8_t rendered[8192],renderwork[8192],normalwork[8192];
 int64_t ids[2048],idstage[2048],scalars[8192],symbols[2048],request[3072],mapwork[512];
 float ropework[65536],image[196608],imagework[196608],normal[5280],decoded[112],published[112];
 uint32_t noise[5280];
};
static double now_seconds(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (double)t.tv_sec+1e-9*(double)t.tv_nsec;}
static int fail(PandaPolicy*p,const char*fmt,...){va_list ap;va_start(ap,fmt);vsnprintf(p->error,sizeof p->error,fmt,ap);va_end(ap);return 0;}
static int effect(PandaPolicy*p,PmStatus status,const char*label){return status?fail(p,"%s: Metal%d %s",label,status,pm_log()):1;}
static int path_join(PandaPolicy*p,char*out,size_t capacity,const char*root,const char*name){int n=snprintf(out,capacity,"%s/%s",root,name);return n>0&&(size_t)n<capacity?1:fail(p,"path extent");}
static unsigned char *read_all(PandaPolicy*p,const char*filename,size_t*bytes){FILE*f=fopen(filename,"rb");if(!f){fail(p,"open %s",filename);return NULL;}if(fseek(f,0,SEEK_END)){fclose(f);fail(p,"seek input");return NULL;}long n=ftell(f);if(n<0||fseek(f,0,SEEK_SET)){fclose(f);fail(p,"input extent");return NULL;}unsigned char*r=malloc((size_t)n+1);if(!r){fclose(f);fail(p,"input allocation");return NULL;}if(fread(r,1,(size_t)n,f)!=(size_t)n){free(r);fclose(f);fail(p,"input read");return NULL;}fclose(f);r[n]=0;*bytes=(size_t)n;return r;}
static void *buffer(PandaPolicy*p,int64_t b){return pm_contents(p->buffers[b]);}
static int inventory_open(PandaPolicy*p,const char*root){
 static const char*names[]={"model-00001-of-00002.safetensors","model-00002-of-00002.safetensors"};p->inventory=calloc(4096,sizeof*p->inventory);coax_t_P5Tensor*draft=calloc(4096,sizeof*draft);if(!p->inventory||!draft){free(draft);return fail(p,"inventory allocation");}
 for(int shard=0;shard<2;shard++){char filename[4096];if(!path_join(p,filename,sizeof filename,root,names[shard])){free(draft);return 0;}int fd=open(filename,O_RDONLY);if(fd<0){free(draft);return fail(p,"original shard open");}struct stat stat;if(fstat(fd,&stat)||stat.st_size<8){close(fd);free(draft);return fail(p,"original shard extent");}p->sizes[shard]=stat.st_size;p->maps[shard]=mmap(NULL,(size_t)stat.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p->maps[shard]==MAP_FAILED){p->maps[shard]=NULL;free(draft);return fail(p,"original shard mmap");}uint64_t header;memcpy(&header,p->maps[shard],8);if(header>1048576||header>(uint64_t)stat.st_size-8){free(draft);return fail(p,"original header extent");}
 coax_t_PwResult r=gca_header((coax_string){(char*)p->maps[shard]+8,(int64_t)header},p->sizes[shard],shard,(coax_v_t_P5Tensor){p->inventory+p->tensors,4096-p->tensors},(coax_v_t_P5Tensor){draft,4096},0);if(r.status){free(draft);return fail(p,"Coax original header%d",r.status);}p->tensors+=r.written;}
 free(draft);return 1;
}
static int allocate_gpu(PandaPolicy*p,const PandaPolicyConfig*config){
 PmDeviceInfo device;if(!effect(p,pm_open(&device),"device"))return 0;int nb=(int)pvn_buffer_count(),nk=(int)pvn_kernel_count();p->buffers=calloc((size_t)nb,sizeof*p->buffers);p->kernels=calloc((size_t)nk,sizeof*p->kernels);p->stages=calloc((size_t)pvn_stage_count(),sizeof*p->stages);if(!p->buffers||!p->kernels||!p->stages)return fail(p,"GPU handles allocation");
 for(int b=0;b<nb;b++){int64_t bytes=pvn_buffer_bytes(p->tokens,b);if(bytes<=0)return fail(p,"buffer%d extent",b);if(!effect(p,pm_alloc((size_t)bytes,p->buffers+b),"resident allocation"))return 0;p->buffers_count++;}
 for(int b=0;b<nb;b++){coax_t_PvnBind r=pvn_bind(b,(coax_v_t_P5Tensor){p->inventory,p->tensors},(coax_v_s64){p->sizes,2});if(r.status)return fail(p,"Coax binding%d refused%d",b,r.status);if(!r.read)continue;unsigned char*src=p->maps[r.shard]+r.begin;void*dst=buffer(p,b);
  if(r.conversion==0)memcpy(dst,src,(size_t)r.bytes);
  else if(r.conversion==1){coax_t_QmResult q=gl_bias(COAX_QmDtype_bf16,r.count,(coax_v_u8){src,r.bytes},(coax_v_f32){dst,r.target/4},0);if(q.status)return fail(p,"Coax vector decode%d",b);}
  else if(r.conversion==2){coax_t_QmResult q=gl_pack(COAX_QmDtype_bf16,r.cols,r.rows,(coax_v_u8){src,r.bytes},(coax_v_u8){dst,r.target},0);if(q.status)return fail(p,"Coax category pack%d",b);}
  else if(r.conversion==3){coax_t_GvpRead q=gac_decode(r.count,(coax_v_u8){src,r.bytes},(coax_v_f32){dst,r.target/4},0);if(q.status)return fail(p,"Coax category decode%d",b);}
  else return fail(p,"unsupported Coax conversion");
 }
 for(int i=0;i<6;i++){size_t bytes=0;unsigned char*source=read_all(p,config->metal_paths[i],&bytes);if(!source)return 0;double seconds;PmStatus s=pm_compile((char*)source,bytes,config->metal_paths[i],p->modules+i,&seconds);free(source);if(!effect(p,s,"compile resident module"))return 0;p->modules_count++;}
 for(int k=0;k<nk;k++){coax_t_GvpKernel d=pvn_kernel(k);if(d.library<0||d.library>=6||d.name.count<=0||d.name.count>=128)return fail(p,"Coax kernel descriptor");char name[128];memcpy(name,d.name.data,(size_t)d.name.count);name[d.name.count]=0;if(!effect(p,pm_kernel(p->modules[d.library],name,p->kernels+k),"resident kernel"))return 0;p->kernels_count++;}
 for(int at=0;at<pvn_stage_count();at++){coax_t_PvnStep s=pvn_cache_stage(p->tokens,at);if(s.status||s.index!=at||s.nodes<=0)return fail(p,"Coax cache stage refused");PolicyStage*c=p->stages+at;c->step=s;c->nodes=calloc((size_t)s.nodes,sizeof*c->nodes);if(!c->nodes)return fail(p,"Coax stage allocation");p->stages_count++;for(int n=0;n<s.nodes;n++){c->nodes[n]=pvn_node(p->tokens,s.graph,s.sequence,n);if(c->nodes[n].status)return fail(p,"Coax node cache refused");}}
 return 1;
}
int panda_policy_open(const PandaPolicyConfig*config,PandaPolicy**out,char*error,size_t capacity){
 if(!config||!out||!config->shard_directory||!config->pst_path||!config->instruction){if(error&&capacity)snprintf(error,capacity,"policy arguments");return 0;}*out=NULL;PandaPolicy*p=calloc(1,sizeof*p);if(!p){if(error&&capacity)snprintf(error,capacity,"policy allocation");return 0;}for(int i=0;i<6;i++)if(!config->metal_paths[i]){fail(p,"missing generated Metal source");goto refused;}
 if(!inventory_open(p,config->shard_directory))goto refused;p->pst=read_all(p,config->pst_path,&p->pst_bytes);if(!p->pst)goto refused;
 coax_t_PvnOpen state={0};coax_v_t_PvnOpen s={&state,1};for(;;){coax_t_PvnOpenStep op=pvn_open_step(state);if(op.status){fail(p,"Coax open descriptor%d",op.status);goto refused;}if(op.kind<0)break;if(op.allocate){p->tokens=state.tokens;if(!allocate_gpu(p,config))goto refused;}
 int status=0;int64_t written=0;
 switch(op.kind){
 case 0:{coax_t_GrRead r=gr_render((coax_string){config->instruction,(int64_t)strlen(config->instruction)},(coax_v_u8){p->rendered,op.output_count},(coax_v_u8){p->renderwork,op.output_count},0);status=r.status;written=r.written;break;}
 case 1:{coax_t_PtResult r=pt_encode_checked((coax_v_u8){p->pst,(int64_t)p->pst_bytes},(coax_string){(char*)p->rendered,op.input_count},(coax_v_s64){p->ids,op.output_count},(coax_v_s64){p->idstage,op.output_count},(coax_v_u8){p->normalwork,8192},(coax_v_s64){p->scalars,8192},(coax_v_s64){p->symbols,2048},0);status=r.status;written=r.written;break;}
 case 2:{coax_t_GrRead r=gr_plan((coax_v_s64){p->ids,op.input_count},0,(coax_v_s64){p->request,op.output_count},0);status=r.status;written=r.written;break;}
 case 3:memcpy(buffer(p,op.destination),p->ids,(size_t)op.output_count*sizeof*p->ids);written=op.receipt;break;
 case 4:{coax_t_GtRead r=gt_images((coax_v_s64){p->ids,op.input_count},(coax_v_s64){buffer(p,op.destination),op.output_count},(coax_v_s64){p->mapwork,op.output_count},0);status=r.status;written=r.written;break;}
 case 5:{coax_t_GtRead r=gt_rope((coax_v_s64){p->request+op.input_offset,op.input_count},(coax_v_f32){buffer(p,op.destination),op.output_count},(coax_v_f32){p->ropework,op.output_count},0);status=r.status;written=r.written;break;}
 case 6:status=gvp_prepare((coax_v_s64){buffer(p,op.destination),op.input_count},(coax_v_u32){buffer(p,op.second),op.output_count});break;
 case 7:status=gad_masks_for(state.tokens,(coax_v_s64){p->ids,op.input_count},(coax_v_u8){buffer(p,op.destination),op.output_count},(coax_v_u8){buffer(p,op.second),op.output_count},0);break;
 default:fail(p,"Coax open operation");goto refused;
 }
 if(pvn_open_ack(s,op.kind,status,written)){fail(p,"Coax open operation%lld refused status%d written%lld",(long long)op.kind,status,(long long)written);goto refused;}}
 p->ready=1;*out=p;return 1;
refused:if(error&&capacity)snprintf(error,capacity,"%s",p->error);panda_policy_close(p);return 0;
}
int panda_policy_capture(PandaPolicy*p,const char*directory){if(!p||!p->ready)return 0;if(!directory){free(p->capture_directory);p->capture_directory=NULL;return 1;}size_t n=strlen(directory);char*q=malloc(n+1);if(!q)return fail(p,"capture path allocation");memcpy(q,directory,n+1);free(p->capture_directory);p->capture_directory=q;return 1;}
static int captures(PandaPolicy*p,coax_t_PvnStep op){if(!p->capture_directory)return 1;for(int slot=0;slot<3;slot++){coax_t_PvnCapture c=pvn_capture(p->tokens,op.graph,op.sequence,slot);if(!c.ok)continue;char name[256],filename[4096];int n;if(c.number>=0)n=snprintf(name,sizeof name,"%.*s%lld%.*s.f32",(int)c.prefix.count,c.prefix.data,(long long)c.number,(int)c.suffix.count,c.suffix.data);else n=snprintf(name,sizeof name,"%.*s%.*s.f32",(int)c.prefix.count,c.prefix.data,(int)c.suffix.count,c.suffix.data);if(n<=0||(size_t)n>=sizeof name||!path_join(p,filename,sizeof filename,p->capture_directory,name))return fail(p,"capture name extent");FILE*f=fopen(filename,"wb");if(!f)return fail(p,"capture create");size_t bytes=(size_t)c.count*4;int ok=fwrite((unsigned char*)buffer(p,c.buffer)+c.offset,1,bytes,f)==bytes;ok=fclose(f)==0&&ok;if(!ok)return fail(p,"capture write");}return 1;}
int panda_policy_forward(PandaPolicy*p,const uint8_t*a,const uint8_t*w,const float*physical,const float*noise,float*out,PandaPolicyTiming*timing){
 if(!p||!p->ready||!a||!w||!physical||!noise||!out)return 0;p->error[0]=0;p->complete=0;PandaPolicyTiming t={0};t.tokens=p->tokens;double begin=now_seconds();coax_t_PvnCursor cursor=pvn_begin(p->tokens);coax_v_t_PvnCursor state={&cursor,1};
 for(;;){coax_t_PvnStep op=pvn_step(cursor);if(op.status)return fail(p,"Coax resident step%d",op.status);if(op.kind==4)break;
 if(op.kind==0){coax_t_PvnPrep d=pvn_prep(op.index);int status=0;int64_t written=0;
  switch(d.kind){case 0:{const uint8_t*raw=d.camera?w:a;coax_t_GiRead r=gi_preprocess(d.first,d.rows,(coax_v_u8){(uint8_t*)raw,d.input_count},(coax_v_f32){p->image,d.output_count},(coax_v_f32){p->imagework,d.output_count},0);status=r.status;written=r.written;break;}
  case 1:{coax_t_GvRead r=gv_pack(d.frames,d.height,d.width,(coax_v_f32){p->image,d.input_count},(coax_v_f32){(float*)buffer(p,d.destination)+d.offset,d.output_count},0);status=r.status;written=r.written;break;}
  case 2:{coax_t_GeRead r=ge_normalize((coax_v_f32){(float*)physical,d.input_count},(coax_v_f32){buffer(p,d.destination),d.output_count},0);status=r.status;written=r.written;break;}
  case 3:{memcpy(p->noise,noise,(size_t)d.input_count*sizeof*p->noise);coax_t_GvpRead r=pvn_noise((coax_v_u32){p->noise,d.input_count},(coax_v_f32){buffer(p,d.destination),d.output_count},0);status=r.status;written=r.written;break;}
  default:return fail(p,"Coax preparation operation");}
  if(pvn_prep_ack(state,op.index,status,written))return fail(p,"Coax preparation%lld refused%d",(long long)op.index,status);if(cursor.phase==0)t.preparation_seconds=now_seconds()-begin;
 }else if(op.kind==1){PolicyStage*c=p->stages+op.index;memset(buffer(p,op.flag_buffer),9,(size_t)op.flags);PmBatch batch;if(!effect(p,pm_begin(&batch),"resident command"))return 0;for(int n=0;n<op.nodes;n++){PmStatus result=groot_gpu_dispatch(batch,c->nodes[n],p->kernels,(size_t)p->kernels_count,p->buffers,(size_t)p->buffers_count);if(pvn_sent(state,n,result)){(void)pm_wait(batch,NULL);return fail(p,"Coax graph%lld stage%lld node%d refused%d",(long long)op.graph,(long long)op.index,n,result);}t.launches++;}
  double seconds=0;PmStatus result=pm_wait(batch,&seconds);if(pvn_waited(state,result,(coax_v_u8){buffer(p,op.flag_buffer),op.flags}))return fail(p,"Coax graph%lld stage%lld completion refused%d %s",(long long)op.graph,(long long)op.index,result,pm_log());if(!captures(p,op))return 0;if(op.graph==0)t.vision_gpu_seconds+=seconds;else if(op.graph==1)t.text_gpu_seconds+=seconds;else t.action_gpu_seconds+=seconds;
 }else if(op.kind==2){memcpy(buffer(p,op.destination),buffer(p,op.source),(size_t)op.bytes);if(pvn_copied(state,0))return fail(p,"Coax handoff refused");
 }else if(op.kind==3){coax_t_GvpRead r=pvn_publish(state,(coax_v_u32){buffer(p,op.source),5280},(coax_v_f32){p->normal,5280},(coax_v_f32){p->decoded,112},(coax_v_f32){p->published,112},0);if(r.status||r.written!=112)return fail(p,"Coax decoded publication refused%d",r.status);
 }else return fail(p,"Coax runtime operation");}
 t.total_seconds=now_seconds()-begin;memcpy(out,p->published,sizeof p->published);if(timing)*timing=t;p->complete=1;return 1;
}
const char *panda_policy_error(const PandaPolicy*p){return p?p->error:"no policy context";}
int panda_policy_copy(PandaPolicy*p,const char*name,void*out,size_t bytes){if(!p||!p->complete||!name||!out)return 0;const void*source=NULL;size_t expected=0;
 if(!strcmp(name,"pixels")){source=buffer(p,330);expected=(size_t)pvn_buffer_bytes(p->tokens,330);}else if(!strcmp(name,"state")){source=buffer(p,1076);expected=(size_t)pvn_buffer_bytes(p->tokens,1076);}else if(!strcmp(name,"noise")){source=p->noise;expected=sizeof p->noise;}else if(!strcmp(name,"ids")){source=p->ids;expected=(size_t)p->tokens*sizeof*p->ids;}else if(!strcmp(name,"backbone")){source=buffer(p,537);expected=(size_t)pvn_buffer_bytes(p->tokens,537);}else if(!strcmp(name,"normalized-actions")){source=p->normal;expected=sizeof p->normal;}else if(!strcmp(name,"decoded-actions")){source=p->published;expected=sizeof p->published;}else return 0;if(bytes!=expected)return 0;memcpy(out,source,bytes);return 1;}
void panda_policy_close(PandaPolicy*p){if(!p)return;for(int i=0;i<p->kernels_count;i++)(void)pm_kernel_free(p->kernels[i]);for(int i=0;i<p->modules_count;i++)(void)pm_unload(p->modules[i]);for(int i=0;i<p->buffers_count;i++)(void)pm_free(p->buffers[i]);for(int i=0;i<p->stages_count;i++)free(p->stages[i].nodes);for(int i=0;i<2;i++)if(p->maps[i])munmap(p->maps[i],(size_t)p->sizes[i]);free(p->capture_directory);free(p->inventory);free(p->pst);free(p->buffers);free(p->kernels);free(p->stages);free(p);}
