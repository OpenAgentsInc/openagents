/* Offline child-effect fixture, never HTTP: consume stdin, write fixed bytes,
 * exit28 or sleep until the real process host cancels this child. */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
int main(int argc,char**argv){
  char input[32];size_t n=0;int c;
  while((c=getchar())!=EOF){if(n<sizeof input)input[n++]=(char)c;}
  if(n!=2||memcmp(input,"{}",2))return 4;
  const char*mode=argc>1?argv[argc-1]:"";
  if(!strcmp(mode,"offline-success")){
    fputs("{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}\n200\n",stdout);
    return 0;
  }
  if(!strcmp(mode,"offline-exit28")){fputs("offline child exit28",stderr);return 28;}
  if(!strcmp(mode,"offline-eof-finish")||!strcmp(mode,"offline-eof-cancel")){
    fputs("{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}\n200\n",stdout);
    fflush(stdout);close(STDOUT_FILENO);
    struct timespec wait={!strcmp(mode,"offline-eof-finish")?3:30,0};
    while(nanosleep(&wait,&wait)){}
    return 0;
  }
  if(!strcmp(mode,"offline-sleep")){
    fputs("sleeping\n",stdout);fflush(stdout);
    struct timespec wait={30,0};while(nanosleep(&wait,&wait)){}
    return 0;
  }
  return 4;
}
