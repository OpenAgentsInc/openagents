// Platform effects only: principal facts, a bounded DB read, the shared Coax codec.
#include "api.h"
#include "chat-history-wire.generated.h"
#include <string.h>
#include <stdlib.h>
static coax_string hs(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
bool chat_history_route(const request *q) {
  if (strcmp(q->path,"/api/chat/history/v1")) return false;
  const auth_principal *p=auth_principal_of(q);
  if (strcmp(q->method,"GET")) {api_error(q->fd,405,"GET this");return true;}
  if (!p || !chh_owns(p->kind,hs(p->account),hs(p->account))) {api_error(q->fd,401,"Log in first.");return true;}
  PGconn *db=api_db();if (!db) {api_error(q->fd,503,"History unavailable.");return true;}
  const char *values[]={p->account};
  PGresult *r=PQexecParams(db,
    "SELECT account,id,title,model,folder,updated_ms::text,busy::text FROM ("
    "SELECT k.account,c.id::text,left(c.title,80) AS title,c.model,coalesce(c.folder,'') AS folder,c.updated_ms,(c.status='running') AS busy "
    "FROM chat.conversations c JOIN auth.nostr_keys k ON k.pubkey=c.owner WHERE k.account=$1 UNION ALL "
    "SELECT account,id,title,'google/gemini-4-hydrogen','',updated_ms,false FROM chat.account_threads_v1 WHERE account=$1"
    ") own ORDER BY updated_ms DESC,id DESC LIMIT 60",
    1,NULL,values,NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK) {PQclear(r);api_error(q->fd,503,"History unavailable.");return true;}
  coax_t_ChatHistoryRow rows[60]={0};int count=PQntuples(r);
  for (int i=0;i<count;i++) {rows[i]=(coax_t_ChatHistoryRow){.owner=hs(PQgetvalue(r,i,0)),.id=hs(PQgetvalue(r,i,1)),.title=hs(PQgetvalue(r,i,2)),.model=hs(PQgetvalue(r,i,3)),.folder=hs(PQgetvalue(r,i,4)),.updated_ms=strtoll(PQgetvalue(r,i,5),NULL,10),.busy=!strcmp(PQgetvalue(r,i,6),"true")};}
  unsigned char out[131072];int64_t n=chh_write(p->kind,hs(p->account),(coax_v_t_ChatHistoryRow){rows,count},(coax_v_u8){out,sizeof out});
  PQclear(r);if (n<0) {api_error(q->fd,503,"History unavailable.");return true;}
  api_respond(q->fd,200,"application/json","Cache-Control: no-store\r\nVary: Cookie, Authorization\r\n",(const char *)out,(size_t)n);return true;
}
