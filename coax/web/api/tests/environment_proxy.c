// Firm reason: 2 (independent C-ABI/process oracle for the Coax gateway adapter).
#include "environment-process.h"
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_string s(const char *p) {return (coax_string){p,(int64_t)strlen(p)};}
int main(int argc,char **argv) {
  if(argc!=6)return 2;
  unsigned char body[24576];
  int64_t made=ea_read_body(s("ListSessions"),s(""),s("0"),(coax_v_u8){body,sizeof body});
  int saved=made>0 && ea_payload_file(s(argv[4]),(coax_string){(const char *)body,made});
  printf("read request bytes: %lld; saved: %d\n",(long long)made,saved);if(!saved)return 1;
  unsigned char *out=malloc(1048576);coax_t_EaView *view=calloc(1,sizeof *view);
  signal(SIGCHLD,SIG_IGN); // production startup/background routes leave this inherited
  int64_t n=environment_proxy(s(argv[1]),s(argv[2]),s("0"),s("wss://relay.openagents.com"),s(argv[3]),s(argv[4]),s(argv[5]),(coax_v_u8){out,1048576});
  printf("environment adapter result: %lld\n",(long long)n);
  int ok=n>0 && ea_view_read((coax_string){(const char *)out,n},(coax_v_t_EaView){view,1});
  if(ok)printf("typed reply rows: %lld; refusal bytes: %lld\n",(long long)view->ns,(long long)view->ne);
  struct sigaction after;sigaction(SIGCHLD,NULL,&after);
  ok=ok && after.sa_handler==SIG_IGN;
  free(view);free(out);return ok?0:1;
}
