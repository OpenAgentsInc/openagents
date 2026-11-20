/* Owned offline curl stand-in for the actual Lab/native-process consumer. */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
int main(void){
 char request[65536];size_t n=fread(request,1,sizeof request,stdin);if(n<3||n==sizeof request||request[0]!='{')return 4;
 const char*mode=getenv("COAX_LAB_TEST_MODE");if(!mode)return 4;
 if(!strcmp(mode,"error")){fputs("offline Lab child exit28",stderr);return 28;}
 if(!strcmp(mode,"not-connected"))return 7;
 if(!strcmp(mode,"busy")){fputs("{\"error\":{\"code\":\"busy\",\"message\":\"another decision is running\"}}\n503\n",stdout);return 0;}
 if(!strcmp(mode,"malformed")){fputs("{\"answers\":{}}\n200\n",stdout);return 0;}
 const char*path=getenv("COAX_LAB_TEST_RESPONSE");FILE*f=path?fopen(path,"rb"):NULL;if(!f)return 4;int c;while((c=fgetc(f))!=EOF)putchar(c);fclose(f);fputs("\n200\n",stdout);fflush(stdout);
 if(!strcmp(mode,"delayed")){close(STDOUT_FILENO);struct timespec wait={30,0};while(nanosleep(&wait,&wait)){};}
 return 0;
}
