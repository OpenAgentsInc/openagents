/* Validate the real sharded index against complete scanned original headers. */
#include "rank5-index-runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 map_file(const char *dir,const char *name){
    char path[4096];int n=snprintf(path,sizeof(path),"%s/%s",dir,name);if(n<0 || (size_t)n>=sizeof(path))exit(2);
    int fd=open(path,O_RDONLY);if(fd<0)exit(2);struct stat st;if(fstat(fd,&st) || st.st_size<=0)exit(2);
    void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)exit(2);return (coax_v_u8){p,st.st_size};
}
int main(int argc,char **argv){
    if(argc!=2)return 2;
    const char *names[2]={"model-00001-of-00002.safetensors","model-00002-of-00002.safetensors"};
    coax_t_PwShard shards[2];memset(shards,0,sizeof(shards));coax_t_P5Tensor *inv=calloc(4096,sizeof(*inv)),*draft=calloc(4096,sizeof(*draft));if(!inv||!draft)return 2;
    int64_t count=0,payload=0;
    for(int i=0;i<2;i++){
        shards[i].name=str(names[i]);shards[i].data=map_file(argv[1],names[i]);
        coax_t_PwResult r=p5_inspect_checked(shards[i].data,(coax_v_t_P5Tensor){inv+count,4096-count},(coax_v_t_P5Tensor){draft,4096},0);
        if(r.status){fprintf(stderr,"scan status %u\n",r.status);return 1;}for(int64_t j=0;j<r.written;j++)inv[count+j].shard=i;count+=r.written;payload+=r.bytes;
    }
    coax_v_u8 index=map_file(argv[1],"model.safetensors.index.json");
    coax_t_PwStatus result=p5_validate_index_test(index,(coax_v_t_PwShard){shards,2},(coax_v_t_P5Tensor){inv,count},payload);
    printf("Mammoth index status=%u tensors=%lld payload=%lld\n",result,(long long)count,(long long)payload);
    return result==COAX_PwStatus_ok && count==1184 && payload==54713457120LL?0:1;
}
