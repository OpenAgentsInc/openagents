/* OS calls and ABI/span checks only. All admission/cleanup policy is Coax. */
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif
#include "read-file-native.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
// Compile-time symbol substitution is used only by the separate native fault
// harness. Production builds leave these as the four actual OS calls.
#ifndef CXFR_OPEN
#define CXFR_OPEN open
#endif
#ifndef CXFR_STAT
#define CXFR_STAT fstat
#endif
#ifndef CXFR_READ
#define CXFR_READ read
#endif
#ifndef CXFR_CLOSE
#define CXFR_CLOSE close
#endif
static int span(const void *data,int64_t count,size_t width){
 if(count<0||(uint64_t)count>SIZE_MAX/width)return 0;
 size_t bytes=(size_t)count*width;if(bytes==0)return 1;
 return data&&(uintptr_t)data<=UINTPTR_MAX-(bytes-1);
}
static int descriptor(int64_t fd){return fd>=INT_MIN&&fd<=INT_MAX;}
int64_t cxfr_open(coax_v_u8 path){
 if(!span(path.data,path.count,1)||path.count>4096)return -EINVAL;
 char terminated[4097];if(path.count)memcpy(terminated,path.data,(size_t)path.count);terminated[path.count]=0;
 int fd=CXFR_OPEN(terminated,O_RDONLY|O_CLOEXEC);if(fd<0){int captured=errno;return -(int64_t)captured;}return fd;
}
int64_t cxfr_stat(int64_t fd,coax_v_s64 metadata){
 if(!descriptor(fd)||metadata.count<2||!span(metadata.data,metadata.count,sizeof(int64_t)))return -EINVAL;
 struct stat info;if(CXFR_STAT((int)fd,&info)<0){int captured=errno;return -(int64_t)captured;}
 _Static_assert(sizeof(info.st_size)<=sizeof(int64_t),"file size ABI");
 metadata.data[0]=(int64_t)info.st_mode;metadata.data[1]=(int64_t)info.st_size;return 0;
}
int64_t cxfr_read(int64_t fd,coax_v_u8 out){
 if(!descriptor(fd)||!span(out.data,out.count,1)||(uint64_t)out.count>SSIZE_MAX)return -EINVAL;
 ssize_t n=CXFR_READ((int)fd,out.data,(size_t)out.count);if(n<0){int captured=errno;return -(int64_t)captured;}return (int64_t)n;
}
int64_t cxfr_close(int64_t fd){
 if(!descriptor(fd))return -EINVAL;
 int status=CXFR_CLOSE((int)fd);if(status<0){int captured=errno;return -(int64_t)captured;}return status;
}
