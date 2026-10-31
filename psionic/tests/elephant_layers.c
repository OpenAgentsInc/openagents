/* The file must first be authenticated by elephant_bundle.c. */
#include "layer-map-memory.h"
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
int main(int argc,char **argv) {
    if(argc!=2) {fprintf(stderr,"usage: elephant-layers MODEL_SAFETENSORS\n");return 2;}
    int fd=open(argv[1],O_RDONLY);if(fd<0){perror("open");return 2;}
    struct stat st;if(fstat(fd,&st) || st.st_size!=18819722392LL)return 2;
    void *map=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(map==MAP_FAILED){perror("mmap");return 2;}
    coax_t_P5Tensor *out=calloc(4096,sizeof(*out)),*draft=calloc(4096,sizeof(*draft));float *params=calloc(100000,sizeof(float)),*work=calloc(100000,sizeof(float));
    if(!out || !draft || !params || !work)return 2;
    coax_v_u8 bytes={map,st.st_size};
    coax_t_PwResult inventory=p5_inspect_checked(bytes,(coax_v_t_P5Tensor){out,4096},(coax_v_t_P5Tensor){draft,4096},0);
    if(inventory.status!=COAX_PwStatus_ok || inventory.written!=760){fprintf(stderr,"inventory status %u\n",inventory.status);return 1;}
    coax_t_PdShape delta={16,32,128,128,0.000001f};coax_t_QfShape attention={16,4,256,64,0.000001f};
    coax_t_QeShape shape={.linear={4096,12288,delta,COAX_QmDtype_bf16},.full={4096,12288,1,attention,COAX_QmDtype_bf16},.state={24,8,1,delta,attention}};
    coax_t_PaFile file={.data=bytes};int linear=0,full=0;
    for(int layer=0;layer<32;layer++) {
        coax_t_PmResult r=psionic_map_layer(shape,layer,(coax_v_t_P5Tensor){out,inventory.written},(coax_v_t_PaFile){&file,1},(coax_v_f32){params,100000},(coax_v_f32){work,100000});
        if(r.status!=COAX_PwStatus_ok){fprintf(stderr,"layer %d status %u\n",layer,r.status);return 1;}
        if((layer+1)%4) {if(r.layer.linear.qkv.count!=67108864 || r.layer.linear.conv.count!=32768)return 1;linear++;}
        else {if(r.layer.full.q.count!=67108864 || r.layer.full.k.count!=8388608)return 1;full++;}
    }
    printf("Elephant mapped %d linear and %d full layers from the original inventory\n",linear,full);
    free(out);free(draft);free(params);free(work);munmap(map,(size_t)st.st_size);return linear==24 && full==8?0:1;
}
