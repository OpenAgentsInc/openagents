#include "rank5-memory.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
int main(int argc,char **argv) {
    if(argc!=2)return 2;
    int fd=open(argv[1],O_RDONLY);if(fd<0)return 3;struct stat st;if(fstat(fd,&st))return 4;
    void *map=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);if(map==MAP_FAILED)return 5;
    coax_t_P5Tensor *out=calloc(4096,sizeof(*out)),*draft=calloc(4096,sizeof(*draft));if(!out||!draft)return 6;
    coax_t_PwResult r=psionic_rank5_inspect((coax_v_u8){map,st.st_size},(coax_v_t_P5Tensor){out,4096},(coax_v_t_P5Tensor){draft,4096});
    printf("status=%u tensors=%lld payload=%lld\n",r.status,(long long)r.written,(long long)r.bytes);
    int found=0;const char *name="model.visual.patch_embed.proj.weight";
    for(int64_t i=0;i<r.written;i++) if(out[i].shape.rank==5) {
        printf("rank5: %.*s [%lld,%lld,%lld,%lld,%lld]\n",(int)out[i].name.count,out[i].name.data,(long long)out[i].shape.dims.e[0],(long long)out[i].shape.dims.e[1],(long long)out[i].shape.dims.e[2],(long long)out[i].shape.dims.e[3],(long long)out[i].shape.dims.e[4]);
        if(out[i].name.count==(int64_t)strlen(name)&&!memcmp(out[i].name.data,name,strlen(name))&&out[i].shape.dims.e[0]==1152&&out[i].shape.dims.e[1]==3&&out[i].shape.dims.e[2]==2&&out[i].shape.dims.e[3]==16&&out[i].shape.dims.e[4]==16)found++;
    }
    int ok=r.status==COAX_PwStatus_ok && r.written==760 && found==1;free(out);free(draft);munmap(map,(size_t)st.st_size);close(fd);return !ok;
}
