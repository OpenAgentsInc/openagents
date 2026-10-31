/* Independent native ABI/metadata oracle. Header-only: no original payload,
   model CPU inference, GPU gate or previously qualified kernel is rerun. */
#include "text-plan.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned cases;
static coax_v_t_GvpCursor S(coax_t_GvpCursor *s){return(coax_v_t_GvpCursor){s,1};}
static unsigned char *read_header(const char *root,const char *name,size_t *bytes){char path[4096];int z=snprintf(path,sizeof path,"%s/%s",root,name);assert(z>0&&(size_t)z<sizeof path);FILE *f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>0&&n<=1048576);rewind(f);unsigned char *p=malloc((size_t)n);assert(p&&fread(p,1,(size_t)n,f)==(size_t)n);assert(!fclose(f));*bytes=(size_t)n;return p;}
static int element_bytes(int kernel,int argument,int buffer){if(buffer==192||buffer==190||buffer==193)return 1;if(buffer==194||buffer==195)return 8;if(kernel==1&&argument==3)return 2;return 4;}
int main(int argc,char **argv){
 assert(argc==3);FILE *facts=fopen(argv[2],"wx");assert(facts);const int64_t sizes[]={4990519232,1919980184};const char *names[]={"model-00001-of-00002.header.json","model-00002-of-00002.header.json"};unsigned char *headers[2];coax_t_P5Tensor *inventory=calloc(4096,sizeof *inventory),*draft=calloc(4096,sizeof *draft);assert(inventory&&draft);int64_t tensors=0;
 for(int shard=0;shard<2;shard++){size_t bytes;headers[shard]=read_header(argv[1],names[shard],&bytes);coax_t_PwResult r=gca_header((coax_string){(char*)headers[shard],(int64_t)bytes},sizes[shard],shard,(coax_v_t_P5Tensor){inventory+tensors,4096-tensors},(coax_v_t_P5Tensor){draft,4096},0);assert(r.status==COAX_PwStatus_ok);tensors+=r.written;}
 assert(tensors==1031&&glp_weight_count()==178&&glp_buffer_count()==202&&glp_kernel_count()==11&&glp_stage_count()==18);
 for(int p=0;p<178;p++){coax_t_GvpSpec spec=glp_spec(p);coax_t_GvpBind r=glp_bind(p,(coax_v_t_P5Tensor){inventory,tensors},(coax_v_s64){(int64_t*)sizes,2});assert(spec.ok&&!r.status&&glp_buffer_bytes(p,140)==r.target_bytes);fprintf(facts,"%d\t%.*s%.*s\t%lld",p,(int)spec.prefix.count,spec.prefix.data,(int)spec.suffix.count,spec.suffix.data,(long long)spec.rank);for(int a=0;a<5;a++)fprintf(facts,"\t%lld",(long long)spec.dims.e[a]);fprintf(facts,"\t%lld\t%lld\t%d\t%lld\t%lld\t%lld\t%lld\t%lld\t%lld\n",(long long)r.bytes,(long long)r.target_bytes,(int)r.decode,(long long)spec.rows,(long long)spec.cols,(long long)r.shard,(long long)r.begin,(long long)(r.begin+r.bytes),(long long)r.count);cases++;}
 const int token_counts[]={137,140,512};for(int variant=0;variant<3;variant++){int tokens=token_counts[variant],nodes=0;for(int stage=0;stage<18;stage++){coax_t_GvpStage s=glp_stage(stage,tokens);assert(!s.status&&s.nodes==(stage==0||stage==17?1:stage<=3?21:20));int64_t offset=0;
  for(int at=0;at<s.nodes;at++){coax_t_GvpNode n=glp_node(stage,at,tokens);assert(!n.status&&n.argc>0&&n.argc<=16&&n.flag_offset==offset);for(int a=0;a<n.argc;a++){coax_t_GvpArg x=n.args.e[a];if(x.kind!=4)continue;assert(x.buffer>=0&&x.buffer<202&&x.offset>=0&&x.count>=0);int64_t capacity=glp_buffer_bytes(x.buffer,tokens);assert(x.offset<=capacity);assert(x.count<=(capacity-x.offset)/element_bytes((int)n.kernel,a,(int)x.buffer));}
   if(n.flag_count){coax_t_GvpArg f=n.args.e[n.argc-1];assert(f.buffer==192&&f.count==n.flag_count&&f.offset==offset);}offset+=n.flag_count;nodes++;cases++;}
  assert(offset==s.flags&&offset<=glp_buffer_bytes(192,tokens));
 }assert(nodes==325);}
 coax_t_GvpCursor cursor={0};for(int stage=0;stage<18;stage++){coax_t_GvpStage meta=glp_stage(stage,140);uint8_t *flags=calloc((size_t)meta.flags,1);assert(flags);for(int at=0;at<meta.nodes;at++)assert(glp_sent(S(&cursor),stage,at,140,0)==COAX_GvpStatus_ok);assert(glp_done(S(&cursor),stage,140,0,(coax_v_u8){flags,meta.flags})==COAX_GvpStatus_ok);assert(cursor.stage==stage+1&&cursor.node==0);free(flags);cases++;}assert(glp_complete(cursor));
 const uint32_t bad[]={0x7f800000,0xff800000,0x7fc00001,0x7f800001};for(int position=0;position<3;position++)for(int i=0;i<4;i++){uint32_t raw[]={0x3f800000,0x80000000,0xc0000000};float out[]={7,7,7,7};raw[position]=bad[i];coax_t_GvpRead r=glp_copy((coax_v_u32){raw,3},(coax_v_f32){out,4},0);assert(r.status==COAX_GvpStatus_number&&r.written==0);for(int a=0;a<4;a++)assert(out[a]==7);cases++;}
 uint32_t raw[]={0x3f800000,0x80000000,0xc0000000};float out[]={7,7,7,7};coax_t_GvpRead copied=glp_copy((coax_v_u32){raw,3},(coax_v_f32){out,4},0);uint32_t negative_zero;memcpy(&negative_zero,&out[1],4);assert(!copied.status&&copied.written==3&&out[0]==1&&negative_zero==0x80000000&&out[2]==-2&&out[3]==7);cases++;
 assert(glp_copy((coax_v_u32){raw,3},(coax_v_f32){out,2},0).status==COAX_GvpStatus_capacity);assert(glp_copy((coax_v_u32){raw,3},(coax_v_f32){out,4},1).status==COAX_GvpStatus_alias);cases+=2;
 for(int effect=1;effect<=7;effect++){cursor=(coax_t_GvpCursor){.node=1};uint8_t flag=0;int expected=effect==7?COAX_GvpStatus_check:COAX_GvpStatus_device;assert(glp_done(S(&cursor),0,140,effect,(coax_v_u8){&flag,1})==expected&&cursor.status==expected);assert(glp_sent(S(&cursor),0,0,140,0)==expected);cases++;}
 cursor=(coax_t_GvpCursor){0};assert(glp_sent(S(&cursor),1,0,140,0)==COAX_GvpStatus_order);cursor=(coax_t_GvpCursor){.stage=-1};assert(glp_sent(S(&cursor),-1,0,140,0)==COAX_GvpStatus_shape);cursor=(coax_t_GvpCursor){0};uint8_t flag=0;assert(glp_done(S(&cursor),0,140,0,(coax_v_u8){&flag,1})==COAX_GvpStatus_order);cases+=3;
 assert(!fclose(facts));for(int i=0;i<2;i++)free(headers[i]);free(inventory);free(draft);printf("GR00T text-plan native: %u independent header/binding/ABI/descriptor/cursor/copy cases passed\n",cases);return 0;
}
