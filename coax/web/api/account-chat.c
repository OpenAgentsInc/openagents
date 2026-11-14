// Platform effects: authenticated facts, parameterized storage and the existing
// metered Vertex adapter. All ownership/funding/shape decisions are Coax.
#include "api.h"
#include "account-chat-wire.generated.h"
#include <openssl/sha.h>
#include <openssl/rand.h>
#include <stdlib.h>
#include <string.h>
static coax_string acs(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static void ac_reply(int fd,const char *body,size_t n) {api_respond(fd,200,"application/json","Cache-Control: no-store\r\nVary: Cookie, Authorization\r\n",body,n);}
static void ac_player(const request *q,const auth_principal *p) {
  if (strcmp(q->method,"POST")) {api_error(q->fd,405,"POST this");return;}
  if (!auth_same_site(q) || !awb_read((coax_string){q->body,(int64_t)q->body_len})) {api_error(q->fd,400,"Invalid player binding.");return;}
  PGconn *db=api_db();if (!db) {api_error(q->fd,503,"Player storage unavailable.");return;}
  const char *v[]={p->verse_account,p->player};
  PQclear(PQexec(db,"BEGIN"));
  PGresult *r=PQexecParams(db,"SELECT id,coalesce(account,'') FROM verse.players WHERE key_sha256=projects.sha($1) FOR UPDATE",1,NULL,&v[1],NULL,NULL,0);
  bool read=PQresultStatus(r)==PGRES_TUPLES_OK,found=read && PQntuples(r)==1;
  bool may=act_bind(p->kind,p->credential==AUTH_COOKIE || p->credential==AUTH_BEARER,acs(p->verse_account),acs(found ? PQgetvalue(r,0,1):""),found,p->limited,p->frozen!=0);
  char id[160]="",before[160]="";if (found) {snprintf(id,sizeof id,"%s",PQgetvalue(r,0,0));snprintf(before,sizeof before,"%s",PQgetvalue(r,0,1));}PQclear(r);
  if (!may) {PQclear(PQexec(db,"ROLLBACK"));api_error(q->fd,read ? 403:503,"This player cannot join that account.");return;}
  const char *a[]={v[0],id,act_bind_guest(acs(before)) ? before:""};
  const char *sql[]={
    "UPDATE verse.players SET account=$1 WHERE id=$2 OR (nullif($3,'') IS NOT NULL AND account=$3)",
    "UPDATE verse.worlds SET owner_account=$1 WHERE (owner_player=$2 AND (owner_account IS NULL OR owner_account=nullif($3,''))) OR (nullif($3,'') IS NOT NULL AND owner_account=$3)",
    "UPDATE verse.progress g SET account=$1 WHERE account=nullif($3,'') AND NOT EXISTS (SELECT 1 FROM verse.progress x WHERE x.account=$1 AND x.slot=g.slot)",
    "DELETE FROM verse.progress WHERE account=nullif($3,'')"};
  Oid types[3]={25,25,25};
  bool ok=true;for (size_t i=0;i<sizeof sql/sizeof sql[0] && ok;i++) {r=PQexecParams(db,sql[i],3,types,a,NULL,NULL,0);ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);}
  if (!ok) {PQclear(PQexec(db,"ROLLBACK"));api_error(q->fd,503,"Player could not join the account.");return;}
  r=PQexecParams(db,"SELECT jsonb_build_object('key',$1::text,'id',$2::text,'account',$3::text)::text",3,NULL,(const char *[]){v[1],id,v[0]},NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK || PQntuples(r)!=1) {PQclear(r);PQclear(PQexec(db,"ROLLBACK"));api_error(q->fd,503,"Player could not join the account.");return;}
  PGresult *commit=PQexec(db,"COMMIT");ok=PQresultStatus(commit)==PGRES_COMMAND_OK;PQclear(commit);
  if (ok) ac_reply(q->fd,PQgetvalue(r,0,0),strlen(PQgetvalue(r,0,0)));else api_error(q->fd,503,"Player could not join the account.");PQclear(r);
}
bool account_chat_route(const request *q) {
  bool world=!strcmp(q->path,"/api/chat/world/complete/v1"),player=!strcmp(q->path,"/api/chat/world/player/v1");
  bool complete=world || !strcmp(q->path,"/api/chat/complete/v1"),thread=!strcmp(q->path,"/api/chat/thread/v1");
  if (!complete && !thread && !player) return false;
  const auth_principal *p=auth_principal_of(q);
  if (!p || !act_owns(p->kind,acs(p->account),acs(p->account))) {api_error(q->fd,401,"Log in first.");return true;}
  if (player) {ac_player(q,p);return true;}
  if (complete) {
    if (strcmp(q->method,"POST")) {api_error(q->fd,405,"POST this");return true;}
    if (!act_funded(p->kind,acs(p->account),p->team)) {api_error(q->fd,403,"Agent chat is currently available to the team beta.");return true;}
    // A namespaced metering subject, never a Nostr key/authenticator. Quotas and
    // usage receipts are inference.c's existing account_day/finish path.
    char subject[96],who[65];unsigned char digest[32];snprintf(subject,sizeof subject,"openagents.account-chat.v1:%s",p->account);
    SHA256((const unsigned char *)subject,strlen(subject),digest);for (int i=0;i<32;i++) snprintf(who+2*i,3,"%02x",digest[i]);
    // Accept the thread profile, then translate with the one gateway codec.
    coax_t_AccountThread *t=calloc(1,sizeof *t);unsigned char body[131072];
    if (!t) {api_error(q->fd,503,"Memory unavailable.");return true;}
    bool ok=act_read((coax_string){q->body,(int64_t)q->body_len},(coax_v_t_AccountThread){t,1});
    int64_t n=world ? awc_completion((coax_string){q->body,(int64_t)q->body_len},(coax_v_u8){body,sizeof body}):ok ? act_completion((coax_v_t_AccountThread){t,1},(coax_v_u8){body,sizeof body}):-1;free(t);
    if (n<1) {api_error(q->fd,400,"Invalid chat request.");return true;}
    request next=*q;next.body=(const char *)body;next.body_len=(size_t)n;return inference_ask(&next,who,true);
  }
  PGconn *db=api_db();if (!db) {api_error(q->fd,503,"Chat storage unavailable.");return true;}
  if (!strcmp(q->method,"GET")) {
    char id[25];api_query(q,"id",id,sizeof id);const char *v[]={p->account,id};
    PGresult *r=PQexecParams(db,"SELECT account,record FROM chat.account_threads_v1 WHERE account=$1 AND id=$2",2,NULL,v,NULL,NULL,0);
    if (PQresultStatus(r)!=PGRES_TUPLES_OK) api_error(q->fd,503,"Chat storage unavailable.");
    else if (PQntuples(r)!=1 || !act_owns(p->kind,acs(p->account),acs(PQgetvalue(r,0,0)))) api_error(q->fd,404,"Chat not found.");
    else ac_reply(q->fd,PQgetvalue(r,0,1),strlen(PQgetvalue(r,0,1)));
    PQclear(r);return true;
  }
  if (strcmp(q->method,"POST")) {api_error(q->fd,405,"GET or POST this");return true;}
  coax_t_AccountThread *t=calloc(1,sizeof *t);unsigned char record[65537];
  if (!t) {api_error(q->fd,503,"Memory unavailable.");return true;}
  if (!act_read((coax_string){q->body,(int64_t)q->body_len},(coax_v_t_AccountThread){t,1})) {free(t);api_error(q->fd,400,"Invalid chat record.");return true;}
  char id[25]={0},revision[32];memcpy(id,t->id.e,(size_t)t->id_n);bool fresh=t->id_n==0;
  if (fresh) {
    unsigned char random[10];if (RAND_bytes(random,sizeof random)!=1) {free(t);api_error(q->fd,503,"ID unavailable.");return true;}
    memcpy(id,"m_",2);for (int i=0;i<10;i++) snprintf(id+2+2*i,3,"%02x",random[i]);
    memcpy(t->id.e,id,22);t->id_n=22;
  }
  int64_t before=t->revision;snprintf(revision,sizeof revision,"%lld",(long long)before);
  t->revision++;int64_t n=act_write((coax_v_t_AccountThread){t,1},(coax_v_u8){record,sizeof record-1});
  // PostgreSQL expects valid UTF-8: let its left() bound the title by characters.
  const char *first=(const char *)t->messages.e[0].text.e;char *full=calloc(1,(size_t)t->messages.e[0].n+1);if (full) memcpy(full,first,(size_t)t->messages.e[0].n);
  free(t);if (n<1 || !full) {free(full);api_error(q->fd,400,"Chat is full. Start a new chat.");return true;}record[n]=0;
  const char *v[]={p->account,id,(const char *)record,revision,full};
  const char *sql=fresh ?
    "INSERT INTO chat.account_threads_v1(account,id,record,revision,title,updated_ms) SELECT $1,$2,$3,1,left($5,80),(extract(epoch from clock_timestamp())*1000)::bigint WHERE $4::bigint=0 ON CONFLICT DO NOTHING RETURNING id" :
    "UPDATE chat.account_threads_v1 SET record=$3,revision=revision+1,title=left($5,80),updated_ms=(extract(epoch from clock_timestamp())*1000)::bigint WHERE account=$1 AND id=$2 AND revision=$4::bigint RETURNING id";
  PGresult *r=PQexecParams(db,sql,5,NULL,v,NULL,NULL,0);free(full);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK) api_error(q->fd,503,"Chat could not be saved.");
  else if (PQntuples(r)!=1) api_error(q->fd,409,"Chat changed on another device. Reopen it before sending.");
  else ac_reply(q->fd,(const char *)record,(size_t)n);
  PQclear(r);return true;
}
