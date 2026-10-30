/* Offline compilation effect: no CUDA device/context required. */
#include <assert.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
typedef void *Program;
int main(int argc,char **argv){
    assert(argc==4);void *lib=dlopen(argv[1],RTLD_NOW|RTLD_LOCAL);assert(lib);
    int(*create)(Program*,const char*,const char*,int,const char*const*,const char*const*)=dlsym(lib,"nvrtcCreateProgram");int(*compile)(Program,int,const char*const*)=dlsym(lib,"nvrtcCompileProgram");int(*log_size)(Program,size_t*)=dlsym(lib,"nvrtcGetProgramLogSize");int(*get_log)(Program,char*)=dlsym(lib,"nvrtcGetProgramLog");int(*ptx_size)(Program,size_t*)=dlsym(lib,"nvrtcGetPTXSize");int(*get_ptx)(Program,char*)=dlsym(lib,"nvrtcGetPTX");int(*destroy)(Program*)=dlsym(lib,"nvrtcDestroyProgram");assert(create&&compile&&log_size&&get_log&&ptx_size&&get_ptx&&destroy);
    FILE *f=fopen(argv[2],"rb");assert(f);fseek(f,0,SEEK_END);long n=ftell(f);rewind(f);char *src=calloc((size_t)n+1,1);assert(src&&fread(src,1,(size_t)n,f)==(size_t)n);fclose(f);Program p;assert(!create(&p,src,"deepseek-prefill.cu",0,NULL,NULL));const char *options[]={"--gpu-architecture=compute_89","--fmad=false","--prec-div=true","--prec-sqrt=true","--ftz=false","--std=c++17"};int status=compile(p,6,options);size_t size=0;assert(!log_size(p,&size));char *log=malloc(size);assert(log&&!get_log(p,log));if(size>1)fputs(log,stderr);assert(!status);assert(!ptx_size(p,&size));char *ptx=malloc(size);assert(ptx&&!get_ptx(p,ptx));f=fopen(argv[3],"wb");assert(f&&fwrite(ptx,1,size,f)==size);fclose(f);assert(!destroy(&p));printf("NVRTC: generated grouped product, flag reduction and guarded publication compile for compute_89 (%zu PTX bytes); no device execution claim\n",size);free(src);free(log);free(ptx);dlclose(lib);return 0;
}
