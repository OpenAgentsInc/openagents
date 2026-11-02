/* Offline child for the canonical typed adapter transcript: no network. */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <string.h>
#include <time.h>
int main(int argc,char**argv){
 char body[3]={0};size_t n=fread(body,1,3,stdin);if(n!=2||memcmp(body,"{}",2))return 4;
 const char*mode=argc>1?argv[argc-1]:"";
 if(!strcmp(mode,"offline-success")){fputs("reply-body\n200\n",stdout);return 0;}
 if(!strcmp(mode,"offline-refused")){fputs("refused-body\n503\n",stdout);return 0;}
 if(!strcmp(mode,"offline-exit7"))return 7;
 if(!strcmp(mode,"offline-exit28")){fputs("offline child exit28",stderr);return 28;}
 if(!strcmp(mode,"offline-sleep")){struct timespec wait={30,0};while(nanosleep(&wait,&wait)){}return 0;}
 return 4;
}
