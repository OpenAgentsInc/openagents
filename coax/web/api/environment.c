// Native effects for the environment profile: account lookup, SQL routing,
// private request file and the Coax broker child. Rules/codecs are Coax.
#include "api.h"
#include "environment-process.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static coax_string es(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static void env_reply(int fd,const void *data,size_t n) {
  api_respond(fd,200,"application/json","Cache-Control: no-store\r\nVary: Cookie, Authorization\r\n",data,n);
}
bool environment_route(const request *q) {
  bool list=!strcmp(q->path,"/api/chat/environments/v1"),call=!strcmp(q->path,"/api/chat/environment/v1");
  if (!list && !call) return false;
  const auth_principal *p=auth_principal_of(q);
  if (!p || !ea_account(p->kind,es(p->account),es(p->account))) {api_error(q->fd,401,"Log in first.");return true;}
  if ((list && strcmp(q->method,"GET")) || (call && strcmp(q->method,"GET") && strcmp(q->method,"POST"))) {api_error(q->fd,405,list ? "GET this":"POST this");return true;}
  PGconn *db=api_db();if (!db) {api_error(q->fd,503,"Connection storage unavailable.");return true;}
  if (!api_limit(db,"environment-call",p->account,(int)ea_rate(),NULL)) {api_error(q->fd,429,"Try again shortly.");return true;}
  char host[65]={0};if (call) api_query(q,"host",host,sizeof host);
  const char *args[]={p->account,host};
  PGresult *r=PQexecParams(db,list ?
    "SELECT host_pubkey,display_name,grant_id,epoch,relay FROM chat.environment_connections_v1 WHERE account=$1 ORDER BY display_name LIMIT 64":
    "SELECT host_pubkey,display_name,grant_id,epoch,relay FROM chat.environment_connections_v1 WHERE account=$1 AND host_pubkey=$2",list ? 1:2,NULL,args,NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK) {PQclear(r);api_error(q->fd,503,"Connection storage unavailable.");return true;}
  if (list) {
    coax_t_EaCatalog *c=calloc(1,sizeof *c);unsigned char out[16384];
    if (!c) {PQclear(r);api_error(q->fd,503,"Memory unavailable.");return true;}
    c->n=PQntuples(r);for (int i=0;i<c->n;i++) {memcpy(c->entries.e[i].host.e,PQgetvalue(r,i,0),64);c->entries.e[i].nl=(int64_t)strlen(PQgetvalue(r,i,1));memcpy(c->entries.e[i].label.e,PQgetvalue(r,i,1),(size_t)c->entries.e[i].nl);}
    int64_t n=ea_catalog_write((coax_v_t_EaCatalog){c,1},(coax_v_u8){out,sizeof out});free(c);PQclear(r);
    if (n<1) api_error(q->fd,503,"Connection list unavailable.");else env_reply(q->fd,out,(size_t)n);return true;
  }
  if (PQntuples(r)!=1) {PQclear(r);api_error(q->fd,404,"Environment not found.");return true;}
  const char *key=getenv("OA_ENV_BROKER_KEY_FILE"),*program=getenv("OA_ENV_BROKER_PROGRAM");
  unsigned char read_body[24576];const char *payload=q->body;size_t payload_len=q->body_len;
  if (!strcmp(q->method,"GET")) {
    char method[33],id[129],before[32];api_query(q,"method",method,sizeof method);api_query(q,"session",id,sizeof id);api_query(q,"before",before,sizeof before);
    int64_t made=ea_read_body(es(method),es(id),es(before),(coax_v_u8){read_body,sizeof read_body});
    if (made<1) {PQclear(r);api_error(q->fd,400,"Unsupported read method.");return true;}payload=(const char *)read_body;payload_len=(size_t)made;
  }
  coax_t_EaRequest *body=calloc(1,sizeof *body);
  if (!body || !ea_read((coax_string){payload,(int64_t)payload_len},(coax_v_t_EaRequest){body,1})) {free(body);PQclear(r);api_error(q->fd,400,"Invalid environment request.");return true;}free(body);
  if (!key || !*key) {PQclear(r);api_error(q->fd,503,"Environment broker unavailable.");return true;}

  char path[]="/tmp/oa-environment-XXXXXX";int f=mkstemp(path);if (f>=0) close(f);
  bool saved=f>=0 && ea_payload_file(es(path),(coax_string){payload,(int64_t)payload_len});
  unsigned char *out=malloc(1048576);int64_t n= -1;
  if (saved && out) n=environment_proxy(es(PQgetvalue(r,0,0)),es(PQgetvalue(r,0,2)),es(PQgetvalue(r,0,3)),es(PQgetvalue(r,0,4)),es(key),es(path),es(program && *program ? program:"/usr/local/bin/environment-host"),(coax_v_u8){out,1048576});
  unlink(path);PQclear(r);
  if (n<1) {api_log("environment broker unavailable: saved=%d result=%lld",saved,(long long)n);api_error(q->fd,503,"Computer unavailable. An uncertain command is never resent automatically.");}else env_reply(q->fd,out,(size_t)n);
  free(out);return true;
}
