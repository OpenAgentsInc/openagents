/* Maps original bytes for Coax; all admission and digest decisions remain there. */
#include "rank5-bundle-memory.h"
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
static coax_string str(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 map_file(const char *dir,const char *name) {
    char path[4096];int n=snprintf(path,sizeof(path),"%s/%s",dir,name);
    if(n<0 || (size_t)n>=sizeof(path)) {fprintf(stderr,"path too long\n");exit(2);}
    int fd=open(path,O_RDONLY);if(fd<0) {perror("open");exit(2);}
    struct stat st;if(fstat(fd,&st) || st.st_size<=0) {perror("fstat");exit(2);}
    void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);
    if(p==MAP_FAILED) {perror("mmap");exit(2);}return (coax_v_u8){p,st.st_size};
}
int main(int argc,char **argv) {
    if(argc!=2) {fprintf(stderr,"usage: elephant-bundle ORIGINAL_DIRECTORY\n");return 2;}
    coax_v_u8 config=map_file(argv[1],"config.json"),tokenizer=map_file(argv[1],"tokenizer.json"),data=map_file(argv[1],"model.safetensors"),none={NULL,0};
    coax_t_PwIdentity identity={str("baselabs/elephant-1"),str("b3f8a93462fb1051a9de031e94a7ff380e254072"),str("f26324589d15932e2742dbb13dbc7c561865ce4f80990af3b7dff05efe4dadab"),str("a5cd9732badce41de57e6efce8302930ded1c1188c5f81feb2bd6c24c4a1941f"),str("")};
    coax_t_PwShard shard={str("model.safetensors"),data,18819722392LL,str("682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba"),0};
    size_t count=sizeof(elephant_specs)/sizeof(*elephant_specs);
    coax_t_P5Spec *specs=calloc(count,sizeof(*specs));coax_t_P5Tensor *out=calloc(count,sizeof(*out)),*draft=calloc(count,sizeof(*draft));
    if(!specs || !out || !draft) {fprintf(stderr,"out of memory\n");return 2;}
    for(size_t i=0;i<count;i++) {
        specs[i].name=str(elephant_specs[i].name);specs[i].shape.rank=elephant_specs[i].rank;
        for(int j=0;j<5;j++)specs[i].shape.dims.e[j]=elephant_specs[i].dims[j];
        specs[i].dtype=COAX_PwDtype_bf16;specs[i].shard=0;
    }
    printf("checking pinned Elephant original: %zu tensors, %lld bytes\n",count,(long long)data.count);fflush(stdout);
    coax_t_PwResult r=psionic_rank5_bundle(identity,config,tokenizer,none,(coax_v_t_PwShard){&shard,1},(coax_v_t_P5Spec){specs,(int64_t)count},(coax_v_t_P5Tensor){out,(int64_t)count},(coax_v_t_P5Tensor){draft,(int64_t)count});
    printf("bundle status=%u tensors=%lld payload=%lld\n",r.status,(long long)r.written,(long long)r.bytes);
    int success=r.status==COAX_PwStatus_ok && r.written==760 && r.bytes==18819627488LL;
    free(specs);free(out);free(draft);munmap(config.data,(size_t)config.count);munmap(tokenizer.data,(size_t)tokenizer.count);munmap(data.data,(size_t)data.count);return success?0:1;
}
