/* Bounded file reads and process status only; Coax validates the complete answer. */
#include "fleet-watch-probe.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static unsigned char request[16384],response[262144];
static coax_string text(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static int read_file(const char*path,unsigned char*out,size_t cap,size_t*n){
 FILE*f=fopen(path,"rb");if(!f)return 0;*n=fread(out,1,cap,f);int extra=fgetc(f);int ok=!ferror(f)&&extra==EOF;fclose(f);return ok;
}
int main(int argc,char**argv){
 if(argc!=6)return 64;size_t n=0,m=0;
 if(!read_file(argv[1],request,sizeof request,&n)||!read_file(argv[2],response,sizeof response,&m))return 65;
 return fw_probe_success((coax_string){(const char*)request,(int64_t)n},(coax_string){(const char*)response,(int64_t)m},text(argv[3]),text(argv[4]),text(argv[5]))?0:2;
}
