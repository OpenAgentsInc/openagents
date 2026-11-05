/* Granite tokenizer and chat renderer on a GGUF file: reads cases written by
   granite_text.coax and prints one line per case for it to compare.
     granite_text MODEL.gguf CASES.bin */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "chat.h"
#include "granite-file.h"
static uint32_t u32(FILE *in){uint8_t b[4];if(fread(b,1,4,in)!=4)return UINT32_MAX;return b[0]|b[1]<<8|b[2]<<16|(uint32_t)b[3]<<24;}
static char *bytes(FILE *in,uint32_t *n){*n=u32(in);char *p=malloc(*n+1);if(*n && fread(p,1,*n,in)!=*n){free(p);return NULL;}p[*n]=0;return p;}
static void ids(GrFile *f,const char *text,int64_t n){
    static int64_t out[1<<16],symbols[1<<12];
    coax_t_GrTokResult r=gr_encode(f->tables,(coax_string){text,n},(coax_v_s64){out,1<<16},(coax_v_s64){symbols,1<<12});
    if(r.status){printf(" refused %u",r.status);return;}
    for(int64_t i=0;i<r.written;i++)printf(" %lld",(long long)out[i]);
    /* Decoding the ids returns the text, byte for byte. */
    static uint8_t back[1<<20];coax_t_GrTokResult d=gr_decode(f->tables,(coax_v_s64){out,r.written},(coax_v_u8){back,1<<20});
    if(d.status || d.written!=n || memcmp(back,text,(size_t)n)){printf(" roundtrip-failed");}
}
int main(int argc,char **argv){
    if(argc!=3){fprintf(stderr,"usage: granite_text MODEL.gguf CASES.bin\n");return 2;}
    GrFile f;char error[256];int code=gr_file_open(argv[1],&f,error,sizeof error);if(code){fprintf(stderr,"%s\n",error);return 1;}
    FILE *in=fopen(argv[2],"rb");if(!in)return 1;
    const char *t=(const char*)f.bytes+f.binding.text.chat;coax_string construct=gr_template_construct((coax_string){t,f.binding.text.chat_bytes});
    printf("template %lld bytes, chapter 52 refuses: %.*s\n",(long long)f.binding.text.chat_bytes,(int)construct.count,construct.data);
    for(;;){
        uint32_t kind=u32(in);if(kind==UINT32_MAX)break;
        if(kind==0){uint32_t n;char *text=bytes(in,&n);printf("T");ids(&f,text,n);printf("\n");free(text);continue;}
        uint8_t flags[3];if(fread(flags,1,3,in)!=3)return 1;
        /* kind 2: tools first, and each message's tool calls */
        uint32_t ntools=0;coax_string *tools=NULL;
        if(kind==2){ntools=u32(in);tools=calloc(ntools+1,sizeof *tools);for(uint32_t t=0;t<ntools;t++){uint32_t n;char *p=bytes(in,&n);tools[t]=(coax_string){p,n};}}
        uint32_t count=u32(in);
        coax_t_GrMessage *m=calloc(count+1,sizeof *m);
        for(uint32_t i=0;i<count;i++){uint32_t a,b;char *role=bytes(in,&a);char *content=bytes(in,&b);m[i].role=(coax_string){role,a};m[i].content=(coax_string){content,b};
            if(kind!=2)continue;
            uint32_t nc=u32(in);coax_t_GrToolCall *calls=calloc(nc+1,sizeof *calls);
            for(uint32_t c=0;c<nc;c++){uint32_t n;char *name=bytes(in,&n);calls[c].name=(coax_string){name,n};uint32_t na=u32(in);coax_t_GrArg *args=calloc(na+1,sizeof *args);
                for(uint32_t q=0;q<na;q++){uint32_t x,y;char *k=bytes(in,&x);char *v=bytes(in,&y);args[q].name=(coax_string){k,x};args[q].value=(coax_string){v,y};}
                calls[c].args=(coax_v_t_GrArg){args,na};}
            m[i].calls=(coax_v_t_GrToolCall){calls,nc};}
        static uint8_t out[1<<20];coax_t_GrChat r=kind==2?gr_chat_tools((coax_v_t_GrMessage){m,count},(coax_v_string){tools,ntools},flags[0],flags[1],flags[2],(coax_v_u8){out,1<<20})
                                                 :gr_chat((coax_v_t_GrMessage){m,count},flags[0],flags[1],flags[2],(coax_v_u8){out,1<<20});
        printf("C");if(r.status){printf(" refused %u\n",r.status);continue;}
        printf(" ");for(int64_t i=0;i<r.written;i++)printf("%02x",out[i]);ids(&f,(const char*)out,r.written);printf("\n");
        for(uint32_t i=0;i<count;i++){free((void*)m[i].role.data);free((void*)m[i].content.data);}free(m);
    }
    gr_file_close(&f);return 0;
}
