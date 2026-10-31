/* Authenticate and bind the original Elephant session through the public API. */
#include "decision-session-memory.h"
#include "elephant-inventory.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 map_file(const char *path){
    int fd=open(path,O_RDONLY);if(fd<0){perror("open");exit(2);}struct stat st;if(fstat(fd,&st)||st.st_size<=0)exit(2);
    void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)exit(2);return (coax_v_u8){p,st.st_size};
}
static coax_v_u8 named(const char *dir,const char *name){char path[4096];int n=snprintf(path,sizeof(path),"%s/%s",dir,name);if(n<0||(size_t)n>=sizeof(path))exit(2);return map_file(path);}
int main(int argc,char **argv){
    if(argc!=3){fprintf(stderr,"usage: elephant-session ORIGINAL_DIRECTORY TOKENIZER_PST\n");return 2;}
    coax_v_u8 config=named(argv[1],"config.json"),tokenizer=named(argv[1],"tokenizer.json"),readout=named(argv[1],"readout.json"),index={NULL,0},table=map_file(argv[2]);
    coax_t_PaFile file={.data=named(argv[1],"model.safetensors")};size_t count=sizeof(elephant_specs)/sizeof(*elephant_specs);
    coax_t_P5Spec *specs=calloc(count,sizeof(*specs));coax_t_P5Tensor *inventory=calloc(count,sizeof(*inventory)),*draft=calloc(count,sizeof(*draft));
    coax_t_QeLayer *layers=calloc(32,sizeof(*layers));float *params=calloc(1057280,sizeof(float)),*stage=calloc(1057280,sizeof(float)),*work=calloc(50000,sizeof(float)),*norm=calloc(4096,sizeof(float)),*norm_work=calloc(4096,sizeof(float));
    if(!specs||!inventory||!draft||!layers||!params||!stage||!work||!norm||!norm_work)return 2;
    for(size_t i=0;i<count;i++){
        specs[i].name=str(elephant_specs[i].name);specs[i].shape.rank=elephant_specs[i].rank;for(int j=0;j<5;j++)specs[i].shape.dims.e[j]=elephant_specs[i].dims[j];
        specs[i].dtype=COAX_PwDtype_bf16;specs[i].shard=0;
    }
    coax_t_PsWorkspace memory={{inventory,(int64_t)count},{draft,(int64_t)count},{layers,32},{params,1057280},{stage,1057280},{work,50000},{norm,4096},{norm_work,4096}};
    puts("authenticating and binding pinned Elephant session");fflush(stdout);
    coax_t_PsSession s=psionic_session_open(0,1,config,tokenizer,readout,index,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,(int64_t)count},table,memory);
    printf("Elephant session status=%u cause=%lld layers=%lld vocabulary=%lld\n",s.status,(long long)s.cause,(long long)s.model.layers.count,(long long)s.model.vocab);
    return s.status==COAX_PsOpenStatus_ok && s.model.layers.count==32 && s.model.vocab==248320 && s.table.data==table.data && s.table.count==table.count?0:1;
}
