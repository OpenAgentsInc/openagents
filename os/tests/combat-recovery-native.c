// Firm reason: 2 (independent oracle: SDK Unix-socket peer checks emitted combat recovery against original recorded lossless sensor strips).
#define _POSIX_C_SOURCE 200809L
#include <poll.h>
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
  REQUIRE(!ferror(in)); REQUIRE(fclose(in)==0 && fclose(out)==0);
}
static long long milliseconds(void) {
  struct timespec t; REQUIRE(clock_gettime(CLOCK_MONOTONIC,&t)==0);
  return (long long)t.tv_sec*1000+t.tv_nsec/1000000;
}
int main(int argc,char **argv) {
  if (argc!=4) { fprintf(stderr,"usage: recovery-native COMBAT_BIN ORIGINAL_STRIP_DIR NEW_ARTIFACT_DIR\n"); return 2; }
  REQUIRE(mkdir(argv[3],0700)==0);
  char temporary[]="/tmp/verse-recovery-peer-XXXXXX"; REQUIRE(mkdtemp(temporary));
  char socket_path[108],run[4096],wire_path[4096],stop[4096];
  REQUIRE(snprintf(socket_path,sizeof socket_path,"%s/socket",temporary)<(int)sizeof socket_path);
  REQUIRE(snprintf(run,sizeof run,"%s/run",argv[3])<(int)sizeof run);
  REQUIRE(snprintf(stop,sizeof stop,"%s/stop",run)<(int)sizeof stop);
  REQUIRE(snprintf(wire_path,sizeof wire_path,"%s/peer-wire.jsonl",argv[3])<(int)sizeof wire_path);
  FILE *wire=fopen(wire_path,"w"); REQUIRE(wire);
  int server=socket(AF_UNIX,SOCK_STREAM,0); REQUIRE(server>=0);
  struct sockaddr_un address={.sun_family=AF_UNIX}; strcpy(address.sun_path,socket_path);
  REQUIRE(bind(server,(struct sockaddr *)&address,sizeof address)==0 && listen(server,1)==0);
  pid_t child=fork(); REQUIRE(child>=0);
  if (!child) {
    close(server); fclose(wire);
    execl(argv[1],argv[1],"--socket",socket_path,"--out",run,"--efficient","--defensive","--home-x","17400","--home-y","51000","--steps","30",(char *)NULL);
    _exit(127);
  }
  struct pollfd ready={.fd=server,.events=POLLIN}; REQUIRE(poll(&ready,1,5000)==1);
  int client=accept(server,NULL,NULL); REQUIRE(client>=0); close(server);
  FILE *input=fdopen(dup(client),"r"),*output=fdopen(client,"w"); REQUIRE(input && output);
  char *line=NULL; size_t capacity=0; int observations=0,bolts=0,recovery_keys=0,held=0,held_code=0; long long pressed_at=0;
  const int expected[2]={30,30};
  for (;;) {
    ready=(struct pollfd){.fd=fileno(input),.events=POLLIN}; REQUIRE(poll(&ready,1,7000)==1);
    ssize_t count=getline(&line,&capacity,input); if (count<0) { REQUIRE(feof(input)); break; }
    REQUIRE(count<8192 && fputs(line,wire)>=0 && fflush(wire)==0);
    if (strstr(line,"\"type\":\"list\"")) {
      REQUIRE(fputs("{\"generation\":2,\"type\":\"windows\",\"session\":7,\"windows\":[{\"handle\":3,\"revision\":5,\"title\":\"World of Warcraft\"}]}\n",output)>=0);
    } else if (strstr(line,"\"type\":\"shot\"")) {
      char *start=strstr(line,"\"path\":\""); REQUIRE(start); start+=8;
      char *end=strchr(start,'"'); REQUIRE(end); *end=0;
      REQUIRE(strncmp(start,run,strlen(run))==0 && start[strlen(run)]=='/');
      if (strstr(start,"/frame.ppm")) observations++;
      REQUIRE(observations>0 && observations<=17);
      char original[4096]; REQUIRE(snprintf(original,sizeof original,"%s/observe-%d.ppm",argv[2],274+observations)<(int)sizeof original);
      copy_frame(original,start);
      REQUIRE(fprintf(output,"{\"generation\":2,\"type\":\"shot\",\"session\":7,\"frame\":%d,\"window\":{\"handle\":3,\"revision\":5,\"width\":1024,\"height\":64}}\n",observations)>0);
    } else {
      if (strstr(line,"\"type\":\"key\"")) {
        char *at=strstr(line,"\"code\":"); REQUIRE(at); int code=(int)strtol(at+7,NULL,10); bool down=strstr(line,"\"down\":true")!=NULL;
        if (code==3) { REQUIRE(recovery_keys==0); if (down) bolts++; }
        else { REQUIRE(recovery_keys<2 && code==expected[recovery_keys] && down==(recovery_keys%2==0)); recovery_keys++; }
        REQUIRE(down?!held:held && held_code==code);
        if (down) { held=1; held_code=code; pressed_at=milliseconds(); }
        else { long long duration=milliseconds()-pressed_at; REQUIRE(duration>=(code==30?80:20) && duration<1500); held=0; }
      } else if (strstr(line,"\"type\":\"stop\"")) {
        REQUIRE(!held);
        if (recovery_keys==2) { FILE *cancel=fopen(stop,"w"); REQUIRE(cancel && fclose(cancel)==0); }
      } else REQUIRE(strstr(line,"\"type\":\"acquire\""));
      REQUIRE(fprintf(output,"{\"generation\":2,\"type\":\"done\",\"session\":7,\"keys\":%d,\"buttons\":0}\n",held)>0);
    }
    REQUIRE(fflush(output)==0);
  }
  free(line); REQUIRE(fclose(input)==0 && fclose(output)==0 && fclose(wire)==0);
  int status=0; REQUIRE(waitpid(child,&status,0)==child && WIFEXITED(status) && WEXITSTATUS(status)==1);
  REQUIRE(bolts>0 && recovery_keys==2 && !held);
  REQUIRE(unlink(socket_path)==0 && rmdir(temporary)==0);
  puts("ok original error-51 strips: eligible defense, one fresh 100 ms facing correction, release and explicit cancellation; no invented damage");
  return 0;
}
