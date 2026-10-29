/* Verify complete model binding after authenticating the original files. */
#include "model-map-memory.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
static coax_v_u8 map_file(const char *path){
    int fd=open(path,O_RDONLY);if(fd<0){perror("open");exit(2);}struct stat st;if(fstat(fd,&st)||st.st_size<=0)exit(2);
    void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)exit(2);return (coax_v_u8){p,st.st_size};
}
int main(int argc,char **argv){
    int mammoth=argc==4 && !strcmp(argv[1],"mammoth"),elephant=argc==3 && !strcmp(argv[1],"elephant");if(!mammoth&&!elephant)return 2;
    int shards=mammoth?2:1;coax_t_PaFile files[2];coax_t_P5Tensor *inventory=calloc(4096,sizeof(*inventory)),*draft=calloc(4096,sizeof(*draft));if(!inventory||!draft)return 2;
    int64_t count=0;
    for(int i=0;i<shards;i++){
        files[i].data=map_file(argv[2+i]);coax_t_PwResult r=p5_inspect_checked(files[i].data,(coax_v_t_P5Tensor){inventory+count,4096-count},(coax_v_t_P5Tensor){draft,4096},0);
        if(r.status)return 1;
        for(int64_t j=0;j<r.written;j++){inventory[count+j].shard=i;}
        count+=r.written;
    }
    coax_t_PdShape delta={16,mammoth?48:32,128,128,0.000001f};coax_t_QfShape att={mammoth?24:16,4,256,64,0.000001f};
    coax_t_QeShape shape={.linear={mammoth?5120:4096,mammoth?17408:12288,delta,COAX_QmDtype_bf16},.full={mammoth?5120:4096,mammoth?17408:12288,1,att,COAX_QmDtype_bf16},.state={mammoth?48:24,mammoth?16:8,1,delta,att}};
    int64_t layers_count=mammoth?64:32,param_count=mammoth?2640384:1057280;
    coax_t_QeLayer *layers=calloc((size_t)layers_count,sizeof(*layers));float *params=calloc((size_t)param_count,sizeof(float)),*stage=calloc((size_t)param_count,sizeof(float)),*work=calloc(100000,sizeof(float));
    if(!layers||!params||!stage||!work)return 2;
    coax_t_PmaResult r=psionic_map_model(shape,(coax_v_t_P5Tensor){inventory,count},(coax_v_t_PaFile){files,shards},(coax_v_t_QeLayer){layers,layers_count},(coax_v_f32){params,param_count},(coax_v_f32){stage,param_count},(coax_v_f32){work,100000});
    printf("%s model binding status=%u layers=%lld parameters=%lld\n",argv[1],r.status,(long long)r.written,(long long)r.parameters);
    return r.status==COAX_PwStatus_ok && r.written==layers_count && r.parameters==param_count?0:1;
}
