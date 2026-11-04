// Firm reason: 2 (independent oracle: a Unix-socket SDK peer replays original fire frames and checks the emitted controller's actual input).
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#define REQUIRE(x) do { if (!(x)) { fprintf(stderr,"oracle failure at line %d: %s\n",__LINE__,#x); exit(1); } } while (0)
static void copy_frame(const char *source,const char *destination) {
  FILE *in=fopen(source,"rb"),*out=fopen(destination,"wb"); REQUIRE(in && out);
  unsigned char bytes[65536]; size_t n;
  while ((n=fread(bytes,1,sizeof bytes,in))) REQUIRE(fwrite(bytes,1,n,out)==n);
  REQUIRE(!ferror(in)); REQUIRE(fclose(in)==0); REQUIRE(fclose(out)==0);
}
static long long milliseconds(void) {
  struct timespec t; REQUIRE(clock_gettime(CLOCK_MONOTONIC,&t)==0);
  return (long long)t.tv_sec*1000+t.tv_nsec/1000000;
}
int main(int argc,char **argv) {
  if (argc!=4) { fprintf(stderr,"usage: health-native NAVIGATE_BIN ORIGINAL_NAV_RUN NEW_ARTIFACT_DIR\n"); return 2; }
  REQUIRE(mkdir(argv[3],0700)==0);
  char temporary[]="/tmp/verse-health-peer-XXXXXX"; REQUIRE(mkdtemp(temporary));
  char socket_path[108],run[4096],wire_path[4096];
  REQUIRE(snprintf(socket_path,sizeof socket_path,"%s/socket",temporary)<(int)sizeof socket_path);
  REQUIRE(snprintf(run,sizeof run,"%s/run",argv[3])<(int)sizeof run);
  REQUIRE(snprintf(wire_path,sizeof wire_path,"%s/peer-wire.jsonl",argv[3])<(int)sizeof wire_path);
  FILE *wire=fopen(wire_path,"w"); REQUIRE(wire);
  int server=socket(AF_UNIX,SOCK_STREAM,0); REQUIRE(server>=0);
  struct sockaddr_un address={.sun_family=AF_UNIX}; strcpy(address.sun_path,socket_path);
  REQUIRE(bind(server,(struct sockaddr *)&address,sizeof address)==0 && listen(server,1)==0);
  pid_t child=fork(); REQUIRE(child>=0);
  if (!child) {
    close(server); fclose(wire);
    execl(argv[1],argv[1],"--socket",socket_path,"--out",run,"--mode","rules","--health-guard","--map","1426","--x","18650","--y","44760","--steps","4",(char *)NULL);
    _exit(127);
  }
  struct pollfd ready={.fd=server,.events=POLLIN}; REQUIRE(poll(&ready,1,5000)==1);
  int client=accept(server,NULL,NULL); REQUIRE(client>=0); close(server);
  FILE *input=fdopen(dup(client),"r"),*output=fdopen(client,"w"); REQUIRE(input && output);
  char *line=NULL; size_t capacity=0; int shots=0,keys=0,stops=0; long long pressed_at=0;
  const int expected_code[4]={17,17,31,31};
  for (;;) {
    ready=(struct pollfd){.fd=fileno(input),.events=POLLIN}; REQUIRE(poll(&ready,1,5000)==1);
    ssize_t count=getline(&line,&capacity,input); if (count<0) { REQUIRE(feof(input)); break; }
    REQUIRE(count<8192); REQUIRE(fputs(line,wire)>=0 && fflush(wire)==0);
    if (strstr(line,"\"type\":\"list\"")) {
      REQUIRE(fputs("{\"generation\":2,\"type\":\"windows\",\"session\":7,\"windows\":[{\"handle\":3,\"revision\":5,\"title\":\"World of Warcraft\"}]}\n",output)>=0);
    } else if (strstr(line,"\"type\":\"shot\"")) {
      char *start=strstr(line,"\"path\":\""); REQUIRE(start); start+=8;
      char *end=strchr(start,'"'); REQUIRE(end); *end=0;
      REQUIRE(strncmp(start,run,strlen(run))==0 && start[strlen(run)]=='/');
      shots++; REQUIRE(shots<=5);
      char original[4096]; int frame=shots<=2?18:shots<=4?19:20;
      REQUIRE(snprintf(original,sizeof original,"%s/observe-%d.ppm",argv[2],frame)<(int)sizeof original);
      copy_frame(original,start);
      REQUIRE(fprintf(output,"{\"generation\":2,\"type\":\"shot\",\"session\":7,\"frame\":%d,\"window\":{\"handle\":3,\"revision\":5,\"width\":2548,\"height\":1428}}\n",shots)>0);
    } else {
      if (strstr(line,"\"type\":\"key\"")) {
        char *code=strstr(line,"\"code\":"); REQUIRE(code && keys<4);
        REQUIRE(strtol(code+7,NULL,10)==expected_code[keys]);
        REQUIRE((strstr(line,"\"down\":true")!=NULL)==(keys%2==0));
        REQUIRE((keys<2 && shots==2) || (keys>=2 && shots==4));
        if (!(keys%2)) pressed_at=milliseconds();
        else { long long held=milliseconds()-pressed_at; REQUIRE(held>=(keys==1?280:480) && held<1500); }
        keys++;
      } else if (strstr(line,"\"type\":\"stop\"")) stops++;
      else REQUIRE(strstr(line,"\"type\":\"acquire\""));
      REQUIRE(fputs("{\"generation\":2,\"type\":\"done\",\"session\":7,\"keys\":0,\"buttons\":0}\n",output)>=0);
    }
    REQUIRE(fflush(output)==0);
  }
  free(line); REQUIRE(fclose(input)==0 && fclose(output)==0 && fclose(wire)==0);
  int status=0; REQUIRE(waitpid(child,&status,0)==child && WIFEXITED(status) && WEXITSTATUS(status)==3);
  REQUIRE(shots==5 && keys==4 && stops==2);
  REQUIRE(unlink(socket_path)==0 && rmdir(temporary)==0);
  puts("ok original fire frames: one forward, fresh health-loss recheck, one 500 ms reverse, releases, final capture; no further forward or inference");
  return 0;
}
