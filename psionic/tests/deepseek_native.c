#define _POSIX_C_SOURCE 200809L
#include "deepseek-memory.h"
#include "deepseek-tiny.h"
#include "text-goldens.h"
#include <assert.h>
#include <stdbool.h>
#include <errno.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <time.h>
#define V(type,p,n) (coax_v_##type){p,n}
static coax_string str(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 map(const char*path){int fd=open(path,O_RDONLY);if(fd<0){perror(path);exit(1);}struct stat s;assert(fstat(fd,&s)==0);void*p=mmap(0,s.st_size,PROT_READ|PROT_WRITE,MAP_PRIVATE,fd,0);close(fd);assert(p!=MAP_FAILED);return V(u8,p,s.st_size);}
static double now(void){struct timespec t;assert(clock_gettime(CLOCK_MONOTONIC,&t)==0);return t.tv_sec+t.tv_nsec*1e-9;}
static void *alloc(int64_t n,size_t w){assert(n>0);void*p=calloc((size_t)n,w);assert(p);return p;}
static void write_all(const char*dir,const char*name,const void*data,size_t width,size_t n){char path[4096];assert(snprintf(path,sizeof path,"%s/%s",dir,name)>0);FILE*f=fopen(path,"wb");assert(f);assert(fwrite(data,width,n,f)==n);assert(fclose(f)==0);}
static coax_t_PtResult chat(coax_v_u8 table,const char*input,int64_t*out){
    coax_t_DsMessage messages[]={{COAX_DsRole_user,str(input)}};int64_t*stage=alloc(16384,8),*temp=alloc(16384,8),*scalars=alloc(262144,8),*symbols=alloc(1024,8);uint8_t*normal=alloc(262144,1);
    coax_t_PtResult r=psionic_ds_chat(table,V(t_DsMessage,messages,1),V(s64,out,16384),V(s64,stage,16384),V(s64,temp,16384),V(u8,normal,262144),V(s64,scalars,262144),V(s64,symbols,1024));
    free(stage);free(temp);free(scalars);free(symbols);free(normal);return r;
}
static void tiny(const char*path){
    coax_v_u8 data=map(path);coax_t_DsBlock blocks[2]={0};coax_t_DsModel m=ds_test_model(data,V(t_DsBlock,blocks,2));coax_t_DsPlan p=ds_plan(m.shape);assert(p.status==COAX_DsStatus_ok);
    float*cache=alloc(p.cache,4),*f=alloc(p.floats,4),logits[10],trace[17];double*scores=alloc(p.scores,8);coax_t_DsWork w={V(f32,f,p.floats),V(f64,scores,p.scores)};coax_t_DsOutput o={V(f32,logits,10),V(f32,trace,17)};logits[9]=-7;trace[16]=-9;
    for(int t=0;t<4;t++){
        coax_t_DsResult r=psionic_ds_step(m,ds_tokens[t],t,V(f32,cache,p.cache),o,w);assert(r.status==COAX_DsStatus_ok&&r.next==t+1&&r.written==9);
        for(int i=0;i<9;i++)assert(fabsf(logits[i]-ds_logits[t][i])<2e-6f);
        for(int i=0;i<16;i++)assert(fabsf(trace[i]-ds_trace[t][i])<2e-6f);
    }assert(logits[9]==-7&&trace[16]==-9);
    // Replaying from an empty cache must reproduce the cached result bit for bit.
    float retained[9];memcpy(retained,logits,sizeof retained);memset(cache,0,(size_t)p.cache*4);
    for(int t=0;t<4;t++)assert(psionic_ds_step(m,ds_tokens[t],t,V(f32,cache,p.cache),o,w).status==COAX_DsStatus_ok);
    assert(memcmp(retained,logits,sizeof retained)==0);
    float*before=alloc(p.cache,4);memcpy(before,cache,(size_t)p.cache*4);memcpy(retained,logits,sizeof retained);
    assert(psionic_ds_step(m,0,4,V(f32,cache,p.cache),(coax_t_DsOutput){V(f32,cache,9),o.trace},w).status==COAX_DsStatus_alias);
    m.p.e[2].data[0]=0x80;m.p.e[2].data[1]=0x7f;
    assert(psionic_ds_step(m,0,4,V(f32,cache,p.cache),o,w).status==COAX_DsStatus_number);
    assert(memcmp(retained,logits,sizeof retained)==0&&memcmp(before,cache,(size_t)p.cache*4)==0);
    m.p.e[2].data[0]=0;m.p.e[2].data[1]=0;cache[0]=NAN;
    assert(psionic_ds_step(m,0,4,V(f32,cache,p.cache),o,w).status==COAX_DsStatus_number);
    free(before);free(cache);free(f);free(scores);munmap(data.data,(size_t)data.count);
    puts("DeepSeek native: independent BF16 layers/logits, exact replay, alias, nonfinite and atomic late failure pass");
}
static void text(const char*path){
    coax_v_u8 table=map(path);assert(ds_tokenizer_admit(table));int64_t*out=alloc(16384,8);coax_t_PtResult r=chat(table,"Hi",out);assert(r.status==COAX_PtStatus_ok);
    printf("DeepSeek chat Hi:");for(int64_t i=0;i<r.written;i++)printf(" %lld",(long long)out[i]);puts("");
    assert(out[0]==151646&&out[1]==151644&&out[r.written-3]==151645&&out[r.written-2]==151648&&out[r.written-1]==198);
    int64_t *stage=alloc(16384,8),*scalars=alloc(262144,8),*symbols=alloc(1024,8);uint8_t*normal=alloc(262144,1);
    for(size_t c=0;c<sizeof(ds_texts)/sizeof(ds_texts[0]);c++){
        coax_t_PtResult e=psionic_ds_encode(table,str(ds_texts[c]),V(s64,out,16384),V(s64,stage,16384),V(u8,normal,262144),V(s64,scalars,262144),V(s64,symbols,1024));
        assert(e.status==COAX_PtStatus_ok&&e.written==ds_text_counts[c]);for(int64_t i=0;i<e.written;i++)assert(out[i]==ds_text_ids[c][i]);
    }
    coax_t_DsMessage history[]={{COAX_DsRole_system,str("Be brief.")},{COAX_DsRole_user,str("Hi")},{COAX_DsRole_assistant,str("<think>private</think>Hello!")},{COAX_DsRole_user,str("1+1?")}};
    int64_t*temp=alloc(16384,8);coax_t_PtResult e=psionic_ds_chat(table,V(t_DsMessage,history,4),V(s64,out,16384),V(s64,stage,16384),V(s64,temp,16384),V(u8,normal,262144),V(s64,scalars,262144),V(s64,symbols,1024));assert(e.status==COAX_PtStatus_ok&&e.written==(int64_t)(sizeof(ds_history_ids)/8));for(int64_t i=0;i<e.written;i++)assert(out[i]==ds_history_ids[i]);
    assert(psionic_ds_encode(table,str("x"),V(s64,out,16384),V(s64,out,16384),V(u8,normal,262144),V(s64,scalars,262144),V(s64,symbols,1024)).status==COAX_PtStatus_alias);
    uint8_t decoded[1024],dscratch[1024];int64_t one=13048;
    assert(psionic_ds_decode(table,V(s64,&one,1),V(u8,decoded,1024),V(u8,dscratch,1024)).status==COAX_PtStatus_ok);
    assert(psionic_ds_decode(table,V(s64,&one,1),V(u8,decoded,1024),V(u8,decoded,1024)).status==COAX_PtStatus_alias);
    free(stage);free(scalars);free(symbols);free(normal);free(temp);
    r=chat(table,"<think>e\xcc\x81 123</think>",out);assert(r.status==COAX_PtStatus_ok);for(int64_t i=2;i<r.written-3;i++)assert(out[i]<151643);
    table.data[100]^=1;assert(!ds_tokenizer_admit(table));free(out);munmap(table.data,(size_t)table.count);
    puts("DeepSeek native: original tokenizer identity, BOS, ordinary-marker isolation and NFC pass");
}
static void original(const char*dir,const char*record,int limit){
    coax_t_DsFile files[5]={0};char path[4096];for(int i=0;i<5;i++){coax_string name=ds_original_filename(i);assert(snprintf(path,sizeof path,"%s/%.*s",dir,(int)name.count,name.data)>0);files[i]=(coax_t_DsFile){name,map(path)};}
    coax_t_DsBlock blocks[28],draft[28];coax_t_DsModel m;double began=now();coax_t_DsResult r=psionic_ds_bind(128,V(t_DsFile,files,5),V(t_DsBlock,blocks,28),V(t_DsBlock,draft,28),V(t_DsModel,&m,1));assert(r.status==COAX_DsStatus_ok);double auth=now()-began;
    if(limit==0){printf("DeepSeek original: all artifact SHA-256 checks and 339 tensor bindings pass; %.3f s\n",auth);for(int i=0;i<5;i++)munmap(files[i].data.data,(size_t)files[i].data.count);return;}
    if(limit<0){
        assert(snprintf(path,sizeof path,"%s/prompt-trace.f32",record)>0);coax_v_u8 hidden=map(path);assert(hidden.count==2*28*1536*4);
        assert(snprintf(path,sizeof path,"%s/prompt-logits.f32",record)>0);coax_v_u8 expected=map(path);assert(expected.count==151936*4);
        float*normal=alloc(1536,4),*out=alloc(151936,4),*stage=alloc(151936,4),*scratch=alloc(151936,4),*x=alloc(1536,4);
        coax_t_DsResult z=ds_test_readout(m,V(f32,(float*)hidden.data+2*28*1536-1536,1536),V(f32,normal,1536),V(f32,out,151936),V(f32,stage,151936),V(f32,scratch,151936));assert(z.status==COAX_DsStatus_ok&&z.written==151936);assert(memcmp(out,expected.data,151936*4)==0);
        z=ds_test_query(m,198,V(f32,x,1536),V(f32,normal,1536),V(f32,out,1536),V(f32,stage,151936),V(f32,scratch,151936));assert(z.status==COAX_DsStatus_ok);write_all(record,"slice-query.f32",out,4,1536);
        printf("DeepSeek original slices: final 151936-row head matches retained logits bit for bit; original layer-0 biased query recorded; authenticated %.3f s\n",auth);
        free(normal);free(out);free(stage);free(scratch);free(x);munmap(hidden.data,(size_t)hidden.count);munmap(expected.data,(size_t)expected.count);for(int i=0;i<5;i++)munmap(files[i].data.data,(size_t)files[i].data.count);return;
    }
    coax_t_DsPlan p=ds_plan(m.shape);float*cache=alloc(p.cache,4),*f=alloc(p.floats,4),*logits=alloc(m.shape.vocab,4),*trace=alloc(2*28*1536,4);double*scores=alloc(p.scores,8);coax_t_DsWork w={V(f32,f,p.floats),V(f64,scores,p.scores)};coax_t_DsOutput o={V(f32,logits,m.shape.vocab),V(f32,trace,2*28*1536)};
    int64_t*tokens=alloc(16384,8);coax_t_PtResult c=chat(files[4].data,"Hi",tokens);assert(c.status==COAX_PtStatus_ok);int64_t prompt=c.written,position=0,generated=0;uint8_t stop=0;began=now();
    while(position<prompt){double t=now();r=psionic_ds_step(m,tokens[position],position,V(f32,cache,p.cache),o,w);assert(r.status==COAX_DsStatus_ok);position=r.next;fprintf(stderr,"prefill %lld/%lld %.3f s\n",(long long)position,(long long)prompt,now()-t);}
    double prefill=now()-began;write_all(record,"prompt-logits.f32",logits,4,(size_t)m.shape.vocab);write_all(record,"prompt-trace.f32",trace,4,2*28*1536);write_all(record,"prompt.i64",tokens,8,(size_t)prompt);
    began=now();
    for(;;){coax_t_DsAdvance a=ds_advance(o.logits,position,m.shape.capacity,generated,limit,false);assert(a.status==COAX_DsStatus_ok);stop=a.stop;if(stop>1)break;tokens[prompt+generated++]=a.token;if(stop==1)break;double t=now();r=psionic_ds_step(m,a.token,position,V(f32,cache,p.cache),o,w);assert(r.status==COAX_DsStatus_ok);position=r.next;fprintf(stderr,"decode %lld token %lld %.3f s\n",(long long)generated,(long long)a.token,now()-t);}
    double decode=now()-began;coax_v_s64 ids=V(s64,tokens+prompt,generated);coax_t_DsParts parts=ds_parts(ids);assert(parts.status==COAX_DsStatus_ok);write_all(record,"generated.i64",ids.data,8,(size_t)ids.count);
    uint8_t*bytes=alloc(65536,1),*scratch=alloc(65536,1);c=ds_decode(files[4].data,ids,V(u8,bytes,65536),V(u8,scratch,65536),false);assert(c.status==COAX_PtStatus_ok);write_all(record,"completion.txt",bytes,1,(size_t)c.written);
    printf("model=DeepSeek-R1-Distill-Qwen-1.5B revision=ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562 profile=deepseek-r1-qwen2-cpu-wide-v1\nauth_s=%.6f prefill_s=%.6f decode_s=%.6f prompt=%lld generated=%lld stop=%u complete=%d cache_bytes=%lld work_bytes=%lld\n",auth,prefill,decode,(long long)prompt,(long long)generated,stop,parts.complete,(long long)p.cache*4,(long long)p.floats*4+(long long)p.scores*8);
    coax_v_s64 reasoning=V(s64,ids.data+parts.reasoning_begin,parts.reasoning_end-parts.reasoning_begin),final=V(s64,ids.data+parts.final_begin,parts.final_end-parts.final_begin);
    c=ds_decode(files[4].data,reasoning,V(u8,bytes,65536),V(u8,scratch,65536),false);assert(c.status==COAX_PtStatus_ok);write_all(record,"reasoning.txt",bytes,1,(size_t)c.written);
    c=ds_decode(files[4].data,final,V(u8,bytes,65536),V(u8,scratch,65536),false);assert(c.status==COAX_PtStatus_ok);write_all(record,"final.txt",bytes,1,(size_t)c.written);printf("final: %.*s\n",(int)c.written,bytes);
    free(bytes);free(scratch);free(tokens);free(cache);free(f);free(logits);free(trace);free(scores);for(int i=0;i<5;i++)munmap(files[i].data.data,(size_t)files[i].data.count);
}
int main(int argc,char**argv){
    if(argc==4&&strcmp(argv[1],"slices")==0){original(argv[2],argv[3],-1);return 0;}
    if(argc==3&&strcmp(argv[1],"bind")==0){original(argv[2],"",0);return 0;}
    if(argc==3&&strcmp(argv[1],"tiny")==0){tiny(argv[2]);return 0;}
    if(argc==3&&strcmp(argv[1],"text")==0){text(argv[2]);return 0;}
    if(argc==5&&strcmp(argv[1],"original")==0){char*end=0;errno=0;long n=strtol(argv[4],&end,10);if(errno||*end||n<1||n>16384){fputs("invalid original-run token limit\n",stderr);return 2;}original(argv[2],argv[3],(int)n);return 0;}
    fprintf(stderr,"usage: %s tiny FILE | text TABLE | original DIRECTORY RECORD_DIRECTORY TOKEN_LIMIT\n",argv[0]);return 2;
}
