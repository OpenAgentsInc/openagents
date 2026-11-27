// Firm reason: 6 (PostgreSQL and authenticated HTTP effects; admission and bytes are shared Coax).
#include "api.h"
#include "account-chat-library-wire.generated.h"
#include "account-chat-wire.generated.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
static void clreply(const request *q,int status,const char *raw,size_t n) {api_respond(q->fd,status,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",raw,n);}
bool account_chat_library_route(const request *q) {
 if(strcmp(q->path,"/api/chat/library/v1"))return false;
 const auth_principal *p=auth_principal_of(q);
 if(!p || !cls_admit(p->kind,p->session[0]!=0,p->limited,p->frozen)){api_error(q->fd,401,"Log in first.");return true;}
 bool post=!strcmp(q->method,"POST");if(!post && strcmp(q->method,"GET")){api_error(q->fd,405,"GET or POST this.");return true;}
 PGconn *db=api_db();if(!db){api_error(q->fd,503,"Sync unavailable.");return true;}
 coax_t_ClWrite *write=calloc(1,sizeof *write);coax_t_ClSnapshot *snapshot=calloc(1,sizeof *snapshot);char *out=malloc(131073);
 if(!write || !snapshot || !out){free(write);free(snapshot);free(out);api_error(q->fd,503,"Sync unavailable.");return true;}
 int status=503;int64_t n=0;PGresult *r=NULL;bool tx=false;const char *v[]={p->account};
 if(post && !cls_read((coax_string){q->body,(int64_t)q->body_len},(coax_v_t_ClWrite){write,1})){status=400;goto done;}
 if(post){r=PQexec(db,"BEGIN");tx=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);r=NULL;if(!tx)goto done;
  r=PQexecParams(db,"SELECT pg_advisory_xact_lock(hashtextextended($1,41107))",1,NULL,v,NULL,NULL,0);if(PQresultStatus(r)!=PGRES_TUPLES_OK)goto done;PQclear(r);r=NULL;
  char command[33];memcpy(command,write->command.e,32);command[32]=0;const char *cv[]={p->account,command};
  r=PQexecParams(db,"SELECT request,response FROM chat.account_library_commands_v1 WHERE account=$1 AND command=$2",2,NULL,cv,NULL,NULL,0);if(PQresultStatus(r)!=PGRES_TUPLES_OK)goto done;
  if(PQntuples(r)){if(strlen(PQgetvalue(r,0,0))!=q->body_len || memcmp(PQgetvalue(r,0,0),q->body,q->body_len)){status=422;goto done;}n=(int64_t)strlen(PQgetvalue(r,0,1));memcpy(out,PQgetvalue(r,0,1),(size_t)n);status=200;goto done;}PQclear(r);r=NULL;
 }
 r=PQexecParams(db,"SELECT revision,document FROM chat.account_library_v1 WHERE account=$1",1,NULL,v,NULL,NULL,0);if(PQresultStatus(r)!=PGRES_TUPLES_OK)goto done;
 if(PQntuples(r)){coax_string body={PQgetvalue(r,0,1),(int64_t)strlen(PQgetvalue(r,0,1))};if(!cls_snapshot_read(body,(coax_v_t_ClSnapshot){snapshot,1}))goto done;}PQclear(r);r=NULL;
 if(!post || !cls_compare(snapshot->revision,write->revision)){n=cls_snapshot_write((coax_v_t_ClSnapshot){snapshot,1},(coax_v_u8){(uint8_t *)out,131072});status=post?409:200;goto done;}
 n=cls_commit((coax_v_t_ClSnapshot){snapshot,1},(coax_v_t_ClWrite){write,1},(coax_v_u8){(uint8_t *)out,131072});if(n<1)goto done;out[n]=0;
 char rev[32],command[33];snprintf(rev,sizeof rev,"%lld",(long long)snapshot->revision);memcpy(command,write->command.e,32);command[32]=0;
 const char *sv[]={p->account,rev,out,command,q->body};
 r=PQexecParams(db,"INSERT INTO chat.account_library_v1(account,revision,document) VALUES($1,$2::bigint,$3) ON CONFLICT(account) DO UPDATE SET revision=$2::bigint,document=$3",3,NULL,sv,NULL,NULL,0);if(PQresultStatus(r)!=PGRES_COMMAND_OK)goto done;PQclear(r);
 const char *receipt[]={p->account,command,q->body,out};
 r=PQexecParams(db,"INSERT INTO chat.account_library_commands_v1(account,command,request,response) VALUES($1,$2,$3,$4)",4,NULL,receipt,NULL,NULL,0);if(PQresultStatus(r)!=PGRES_COMMAND_OK)goto done;PQclear(r);r=NULL;
 {
  long long now=(long long)api_now_ms();char ms[32];snprintf(ms,sizeof ms,"%lld",now);
  coax_t_CcAcctEvent aev={0};
  aev.event_type=(coax_string){"library_updated",15};
  aev.conversation=(coax_string){"",0};
  aev.turn=(coax_string){"",0};
  aev.title=(coax_string){"",0};
  aev.compute=(coax_string){"",0};
  aev.model=(coax_string){"",0};
  aev.source_device=(coax_string){"",0};
  aev.revision=snapshot->revision;
  aev.updated_at_ms=now;
  uint8_t evbuf[1024];
  int64_t evn=cc_acct_event_write((coax_v_t_CcAcctEvent){&aev,1},(coax_v_u8){evbuf,(int64_t)(sizeof evbuf-1)});
  if(evn>0){
   evbuf[evn]=0;
   const char *av[]={p->account,(char *)evbuf,ms};
   r=PQexecParams(db,"INSERT INTO chat.account_events_v2(account,conversation,event_type,record,created_ms) VALUES($1,'','library_updated',$2,$3::bigint)",3,NULL,av,NULL,NULL,0);
   if(PQresultStatus(r)!=PGRES_COMMAND_OK)goto done;
  }
 }
 status=200;
 done:if(r){if(status==503){const char *state=PQresultErrorField(r,PG_DIAG_SQLSTATE);api_log("chat library storage SQLSTATE=%.5s",state?state:"none");}PQclear(r);}if(tx){r=PQexec(db,status==200?"COMMIT":"ROLLBACK");if(PQresultStatus(r)!=PGRES_COMMAND_OK)status=503;PQclear(r);}
 if((status==200 || status==409) && n>0)clreply(q,status,out,(size_t)n);else api_error(q->fd,status,"Library sync unavailable.");free(write);free(snapshot);free(out);return true;
}
