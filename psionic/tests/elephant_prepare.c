/* Compare public Coax request preparation with the real experiment token IDs. */
#include "decision-prepare-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
int main(int argc,char **argv){
    if(argc!=3)return 2;
    int fd=open(argv[1],O_RDONLY);if(fd<0)return 2;struct stat st;if(fstat(fd,&st)||st.st_size<=0)return 2;
    void *table=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(table==MAP_FAILED)return 2;
    coax_v_u8 tv={table,st.st_size};assert(pt_admit(tv));
    unsigned char *prompt=calloc(65536,1),*render_work=calloc(65536,1),*normal=calloc(262144,1);
    int64_t *candidate=calloc(16384,sizeof(int64_t)),*full=calloc(16384,sizeof(int64_t)),*stage=calloc(16384,sizeof(int64_t)),*scalars=calloc(262144,sizeof(int64_t)),*symbols=calloc(1024,sizeof(int64_t)),*out=calloc(16384,sizeof(int64_t));
    if(!prompt||!render_work||!normal||!candidate||!full||!stage||!scalars||!symbols||!out)return 2;
    coax_t_PrWorkspace work={{prompt,65536},{render_work,65536},{candidate,16384},{full,16384},{stage,16384},{normal,262144},{scalars,262144},{symbols,1024}};
    coax_t_PtEntry state={COAX_PtEntryKind_text,str("Water is wet.")};coax_t_PtQuestion q={COAX_PtQuestionKind_choice,{COAX_PtEntryKind_text,str("Is the water wet?")}};
    coax_t_PtOption opts[2]={{str("yes"),{COAX_PtEntryKind_none,str("")}},{str("no"),{COAX_PtEntryKind_none,str("")}}};
    coax_t_PrResult r=psionic_prepare(tv,state,q,(coax_v_t_PtOption){opts,2},(coax_v_s64){out,16384},work);
    if(r.status){fprintf(stderr,"prepare status %u cause %lld\n",r.status,(long long)r.cause);return 1;}
    FILE *ref=fopen(argv[2],"r");if(!ref)return 2;long long value;if(fscanf(ref,"%lld",&value)!=1 || value!=r.prefix)return 1;
    for(int64_t i=0;i<r.written;i++){if(fscanf(ref,"%lld",&value)!=1||value!=out[i])return 1;}if(fscanf(ref,"%lld",&value)==1)return 1;fclose(ref);
    printf("real Qwen request: %lld tokens, prefix %lld, suffix %lld, exact match\n",(long long)r.written,(long long)r.prefix,(long long)(r.written-r.prefix));return 0;
}
