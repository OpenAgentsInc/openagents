#define _POSIX_C_SOURCE 200809L
/* Resolver fault injection for tests only. The real Coax address classifier
   remains linked; no observation or policy result is supplied by this shim. */
#include <arpa/inet.h>
#include <netdb.h>
#include <stdio.h>
#include <unistd.h>
#include <string.h>
#include <time.h>
static struct addrinfo rows[17];
static struct sockaddr_in v4[17];
static struct sockaddr_in6 v6;
static struct sockaddr unsupported;
int fi_test_getaddrinfo(const char *host,const char *port,const struct addrinfo *hints,struct addrinfo **out) {
 if(strcmp(host,"stall.fixture")==0){
#ifdef FI_TEST_PID_FILE
  FILE *file=fopen(FI_TEST_PID_FILE,"w");if(file){fprintf(file,"%ld",(long)getpid());fclose(file);}
#endif
  struct timespec t={30,0};nanosleep(&t,0);return EAI_NONAME;
 }
 int n=0,mode=0;
 if(strcmp(host,"mixed.fixture")==0){n=2;mode=1;}
 else if(strcmp(host,"oversize.fixture")==0){n=17;}
 else if(strcmp(host,"family.fixture")==0){n=1;mode=2;}
 else if(strcmp(host,"mapped.fixture")==0){n=1;mode=3;}
 else if(strcmp(host,"loopback.fixture")==0){n=1;mode=4;}
 else if(strcmp(host,"short.fixture")==0){n=1;mode=5;}
 else if(strcmp(host,"wrongfam.fixture")==0){n=1;mode=6;}
 else return getaddrinfo(host,port,hints,out);
 memset(rows,0,sizeof(rows));memset(v4,0,sizeof(v4));
 for(int i=0;i<n;i++){
  v4[i].sin_family=AF_INET;v4[i].sin_port=htons(443);inet_pton(AF_INET,(mode==4 || (mode==1 && i==1))?"127.0.0.1":"8.8.8.8",&v4[i].sin_addr);
  rows[i].ai_family=AF_INET;rows[i].ai_socktype=SOCK_STREAM;rows[i].ai_protocol=IPPROTO_TCP;rows[i].ai_addr=(struct sockaddr *)&v4[i];rows[i].ai_addrlen=sizeof(v4[i]);
  if(i+1<n)rows[i].ai_next=&rows[i+1];
 }
 if(mode==2){memset(&unsupported,0,sizeof(unsupported));unsupported.sa_family=AF_UNIX;rows[0].ai_family=AF_UNIX;rows[0].ai_addr=&unsupported;rows[0].ai_addrlen=sizeof(unsupported);}
 if(mode==3){memset(&v6,0,sizeof(v6));v6.sin6_family=AF_INET6;v6.sin6_port=htons(443);inet_pton(AF_INET6,"::ffff:8.8.8.8",&v6.sin6_addr);rows[0].ai_family=AF_INET6;rows[0].ai_addr=(struct sockaddr *)&v6;rows[0].ai_addrlen=sizeof(v6);}
 if(mode==5)rows[0].ai_addrlen=0;
 if(mode==6)rows[0].ai_family=AF_INET6;
 *out=rows;return 0;
}
void fi_test_freeaddrinfo(struct addrinfo *a) {if(a!=rows)freeaddrinfo(a);}
