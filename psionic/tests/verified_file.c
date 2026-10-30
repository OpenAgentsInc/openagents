#define _GNU_SOURCE
#include "serve.h"
#include "verified-file.h"
#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
int main(void){
    char dir[]="/tmp/psionic-vf.XXXXXX";assert(mkdtemp(dir));char path[256],old[256];snprintf(path,sizeof path,"%s/weights",dir);snprintf(old,sizeof old,"%s/old",dir);
    int fd=open(path,O_RDWR|O_CREAT|O_EXCL,0600);assert(fd>=0 && write(fd,"old!",4)==4);struct stat before;assert(!fstat(fd,&before));
    void *mapped=mmap(NULL,4,PROT_READ,MAP_PRIVATE,fd,0);assert(mapped!=MAP_FAILED);assert(!rename(path,old));int fresh=open(path,O_WRONLY|O_CREAT|O_EXCL,0600);assert(fresh>=0 && write(fresh,"new!",4)==4);close(fresh);
    coax_t_PaFile f={.data={mapped,4}};VfEntry e;vf_attach_fd(&e,fd,&f);assert(f.stat.inode==(int64_t)before.st_ino && !memcmp(f.data.data,"old!",4));close(fd);munmap(mapped,4);
    coax_t_PaFile next={0};assert(vf_map_file(&e,path,&next)==0 && next.stat.inode!=(int64_t)before.st_ino && !memcmp(next.data.data,"new!",4));munmap(next.data.data,(size_t)next.data.count);
    assert(vf_map_file(&e,"/nonexistent/psionic-check",&next)!=0);unlink(path);unlink(old);rmdir(dir);puts("verified files: identity comes from the mapped descriptor across path replacement");
}
