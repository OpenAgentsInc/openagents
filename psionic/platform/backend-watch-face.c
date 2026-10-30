/* Clock, file and process argument effects only. Coax owns every decision. */
#define _POSIX_C_SOURCE 200809L
#include "backend-watch.h"
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
static coax_string text(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static int64_t read_file(const char*path,unsigned char*data,size_t cap,int optional){
 FILE*f=fopen(path,"rb");if(!f)return optional&&errno==ENOENT?0:-1;
 size_t n=fread(data,1,cap+1,f);int bad=ferror(f);fclose(f);return bad||n>cap?-1:(int64_t)n;
}
int main(int argc,char**argv){
 unsigned char out[8192],body[8193],prior[8193];int64_t n=-1;
 if(argc==2&&!strcmp(argv[1],"clock")){struct timespec t;if(clock_gettime(CLOCK_MONOTONIC,&t))return 1;printf("{\"wall\":%lld,\"mono\":%lld}\n",(long long)time(NULL),(long long)t.tv_sec);return 0;}
 if(argc==5&&!strcmp(argv[1],"window")){printf("%s\n",bw_window_raw(text(argv[2]),text(argv[3]),text(argv[4]))?"true":"false");return 0;}
 if(argc==3&&!strcmp(argv[1],"describe"))n=bw_describe(text(argv[2]),(coax_v_u8){out,sizeof out});
 if(argc==14&&!strcmp(argv[1],"observe")){
  int64_t b=read_file(argv[12],body,8192,0),p=read_file(argv[13],prior,8192,1);if(b<0||p<0)return 1;
  coax_string args[10];for(int i=0;i<10;i++)args[i]=text(argv[i+2]);
  n=bw_run((coax_v_string){args,10},(coax_string){(char*)body,b},(coax_string){(char*)prior,p},(coax_v_u8){out,sizeof out});
 }
 if(n<0)return 2;if(fwrite(out,1,(size_t)n,stdout)!=(size_t)n||fputc('\n',stdout)==EOF)return 1;return 0;
}
