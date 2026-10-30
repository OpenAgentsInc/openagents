/* Argument/stdout effects only. All target/window/recovery decisions are Coax. */
#include "fleet-watch.h"
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_string text(const char *s){return(coax_string){s,(int64_t)strlen(s)};}
static int number(const char*s,int64_t*out){char*end=NULL;errno=0;long long n=strtoll(s,&end,10);if(errno||!s[0]||*end)return 0;*out=(int64_t)n;return 1;}
int main(int argc,char**argv){
 int64_t now,start,end;
 if(argc==5&&!strcmp(argv[1],"window")){
  if(!number(argv[2],&now)||!number(argv[3],&start)||!number(argv[4],&end))return 64;
  printf("%lld\n",(long long)fw_window(now,start,end));return 0;
 }
 if(argc==10&&!strcmp(argv[1],"decision")){
  if(!number(argv[6],&now)||!number(argv[7],&start)||!number(argv[8],&end))return 64;
  printf("%lld\n",(long long)fw_plan(text(argv[2]),text(argv[3]),text(argv[4]),text(argv[5]),now,start,end,text(argv[9])));return 0;
 }
 return 64;
}
