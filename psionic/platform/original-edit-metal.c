/* Chapter 255 experiment effects. Coax owns model/window admission, all
   arithmetic, prompts, token selection, stopping, metrics and the matrix.
   Files map read-only. Patches affect a private, idle test session only. */
#include "gptoss-device.h"
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
static PgSession *oe_session;
static coax_t_GFile oe_files[10];
static coax_v_u8 oe_map(coax_string path){
    char name[4096];if(path.count<1 || path.count>=4096)return (coax_v_u8){0};
    for(int64_t i=0;i<path.count;i++)name[i]=path.data[i];name[path.count]=0;
    int fd=open(name,O_RDONLY);if(fd<0)return (coax_v_u8){0};struct stat st;
    if(fstat(fd,&st) || st.st_size<1){close(fd);return (coax_v_u8){0};}
    void *data=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);
    if(data==MAP_FAILED)return (coax_v_u8){0};return (coax_v_u8){data,st.st_size};
}
int64_t oe_open(coax_v_u8 root,int64_t model,int64_t capacity,int64_t budget,coax_v_u8 source){
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];
    coax_t_GProfile p=g_profile((uint8_t)model,capacity,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});
    if(p.status)return 1000+p.status;
    for(int64_t i=0;i<p.pins.count;i++){
        char path[4096];int n=snprintf(path,sizeof path,"%.*s/%.*s",(int)root.count,root.data,(int)p.pins.data[i].name.count,p.pins.data[i].name.data);
        if(n<0 || n>=4096)return -1;
        oe_files[i]=(coax_t_GFile){p.pins.data[i].name,oe_map((coax_string){path,n})};
    }
    coax_t_GResult why;PcStatus status=pg_open(p,(coax_v_t_GFile){oe_files,p.pins.count},(char*)source.data,(size_t)source.count,(size_t)budget,&oe_session,&why);
    if(status){fprintf(stderr,"Metal open platform=%d\n",status);return status;}
    if(why.status){fprintf(stderr,"Coax model admission=%u\n",why.status);return 1000+why.status;}return 0;
}
int64_t oe_reset(void){return pg_reset(oe_session);}
int64_t oe_prefill(coax_v_s64 tokens,coax_v_s64 control){return pg_prefill(oe_session,tokens.data,tokens.count,0,0,128,control.data);}
int64_t oe_step(int64_t token,int64_t position,coax_v_s64 control){return pg_step(oe_session,token,position,0,128,1,control.data);}
int64_t oe_read(int64_t slot,coax_v_f32 values){return pg_read(oe_session,(int)slot,values.data,(size_t)values.count*4);}
int64_t oe_patch(int64_t offset,coax_v_u8 bytes){return pg_patch(oe_session,(size_t)offset,bytes.data,(size_t)bytes.count);}
int64_t oe_original(int64_t file,int64_t offset,coax_v_u8 out){
    if(file<0 || file>=10 || offset<0 || offset>oe_files[file].data.count || out.count>oe_files[file].data.count-offset)return -1;
    memcpy(out.data,oe_files[file].data.data+offset,(size_t)out.count);return 0;
}
int64_t oe_close(void){pg_close(oe_session);oe_session=NULL;for(int i=0;i<10;i++)if(oe_files[i].data.data){munmap(oe_files[i].data.data,(size_t)oe_files[i].data.count);oe_files[i].data=(coax_v_u8){0};}return 0;}
int64_t oe_bytes(coax_v_f32 values,coax_v_u8 out){if(out.count!=values.count*4)return -1;memcpy(out.data,values.data,(size_t)out.count);return 0;}
