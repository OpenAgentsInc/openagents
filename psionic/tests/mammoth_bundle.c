/* Maps original bytes for Coax; all admission and digest decisions remain there. */
#include "rank5-bundle-memory.h"
#include "mammoth-inventory.h"
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
    if(argc!=2) {fprintf(stderr,"usage: mammoth-bundle ORIGINAL_DIRECTORY\n");return 2;}
    coax_v_u8 config=map_file(argv[1],"config.json"),tokenizer=map_file(argv[1],"tokenizer.json"),index=map_file(argv[1],"model.safetensors.index.json");
    coax_v_u8 data1=map_file(argv[1],"model-00001-of-00002.safetensors"),data2=map_file(argv[1],"model-00002-of-00002.safetensors");
    coax_t_PwIdentity identity={str("baselabs/mammoth-1"),str("61cec44c281c693992e3d6ebeb44dd3402dfa186"),str("bab5a8ceb73fd6a8f191afaf23afbf689d931da5b3c7c9d5153cf1e4b5024502"),str("a5cd9732badce41de57e6efce8302930ded1c1188c5f81feb2bd6c24c4a1941f"),str("0424520f63f04816eee4d229a4487dc9744866ceb46513e591dd987e401b1663")};
    coax_t_PwShard shards[2]={
      {str("model-00001-of-00002.safetensors"),data1,49825162976LL,str("29701f9a7b131bedb3fe1d565dad69b912d9128c6fc27e3d8fb0aedc47ae0e06")},
      {str("model-00002-of-00002.safetensors"),data2,4888445168LL,str("4da0ff5476813bbd9898454f0d7f7ba3c82914c42e32e2b87577c6d69daae671")}};
    size_t count=sizeof(mammoth_specs)/sizeof(*mammoth_specs);
    coax_t_P5Spec *specs=calloc(count,sizeof(*specs));coax_t_P5Tensor *out=calloc(count,sizeof(*out)),*draft=calloc(count,sizeof(*draft));
    if(!specs || !out || !draft) {fprintf(stderr,"out of memory\n");return 2;}
    for(size_t i=0;i<count;i++) {
        specs[i].name=str(mammoth_specs[i].name);specs[i].shape.rank=mammoth_specs[i].rank;
        for(int j=0;j<5;j++)specs[i].shape.dims.e[j]=mammoth_specs[i].dims[j];
        specs[i].dtype=COAX_PwDtype_bf16;specs[i].shard=mammoth_specs[i].shard;
    }
    printf("checking pinned Mammoth originals: %zu tensors, %lld bytes\n",count,(long long)(data1.count+data2.count));fflush(stdout);
    coax_t_PwResult r=psionic_rank5_bundle(identity,config,tokenizer,index,(coax_v_t_PwShard){shards,2},(coax_v_t_P5Spec){specs,(int64_t)count},(coax_v_t_P5Tensor){out,(int64_t)count},(coax_v_t_P5Tensor){draft,(int64_t)count});
    printf("bundle status=%u tensors=%lld payload=%lld\n",r.status,(long long)r.written,(long long)r.bytes);
    int success=r.status==COAX_PwStatus_ok && r.written==1184 && r.bytes==54713457120LL;
    free(specs);free(out);free(draft);munmap(config.data,(size_t)config.count);munmap(tokenizer.data,(size_t)tokenizer.count);munmap(index.data,(size_t)index.count);munmap(data1.data,(size_t)data1.count);munmap(data2.data,(size_t)data2.count);return success?0:1;
}
