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
// Account task controls use the same broker and independent native HOST grant.
// Connection exclusivity and the owned conversation/turn precede every write.
// Start belongs to server-owned routing/context admission, never this bridge.
static bool environment_task_route(const request *q) {
  bool transcript=!strcmp(q->path,"/api/chat/task/transcript/v1alpha1");
  bool models=!strcmp(q->path,"/api/chat/task/models/v1alpha1");
  if(!transcript&&!models&&strcmp(q->path,"/api/chat/task/v1alpha1"))return false;
  const auth_principal *p=auth_principal_of(q);
  if(!p||!ea_account(p->kind,es(p->account),es(p->account))){api_error(q->fd,401,"Log in first.");return true;}
  bool post=!strcmp(q->method,"POST");if(!post&&strcmp(q->method,"GET")){api_error(q->fd,405,"GET or POST this");return true;}
  if((transcript||models)&&post){api_error(q->fd,405,"GET this");return true;}
  if(post&&!auth_same_site(q)){api_error(q->fd,403,"Cross-site write refused.");return true;}
  // Keep one extra byte so the shared query reader cannot turn an overlong
  // reference into a valid truncated host/task prefix.
  char host[66]={0},task[34]={0};api_query(q,"host",host,sizeof host);
  unsigned char request_arena[1024],read_body[1024];coax_t_XtRequest control={0};
  if(post){if(!ta_request((coax_string){q->body,(int64_t)q->body_len},(coax_v_t_XtRequest){&control,1},(coax_v_u8){request_arena,sizeof request_arena})){api_error(q->fd,400,"Invalid task control. Dispatch is server-owned.");return true;}
    memcpy(task,control.task.data,32);
  }else{api_query(q,"task",task,sizeof task);}
  coax_t_XoRequest offer_request={0};unsigned char offer_request_arena[128];
  int64_t read_n=ta_read(es(task),(coax_v_u8){read_body,sizeof read_body});
  if(models){char probe[34]={0},engine[10]={0},account[34]={0};api_query(q,"probe",probe,sizeof probe);api_query(q,"engine",engine,sizeof engine);api_query(q,"provider_account",account,sizeof account);
    coax_t_XoRequest params={.probe=es(probe),.engine=es(engine),.account=es(account)};read_n=xo_request_write(params,(coax_v_u8){read_body,sizeof read_body});
    if(read_n<1||!xo_request_read((coax_string){(const char *)read_body,read_n},(coax_v_t_XoRequest){&offer_request,1},(coax_v_u8){offer_request_arena,sizeof offer_request_arena})){api_error(q->fd,400,"Invalid native engine, provider account or probe.");return true;}
  }
  if(!ta_host(es(host))||read_n<1){api_error(q->fd,400,"Invalid computer or task identity.");return true;}
  unsigned char transcript_body[1024],transcript_request_arena[256];int64_t transcript_n=0;coax_t_XhRequest transcript_request={0};
  if(transcript){char reader[34]={0},read[34]={0},hash[66]={0},offset[32]={0};api_query(q,"reader",reader,sizeof reader);api_query(q,"read",read,sizeof read);api_query(q,"sha256",hash,sizeof hash);api_query(q,"after",offset,sizeof offset);
    transcript_n=xh_read_body(es(task),es(reader),es(read),es(hash),es(offset),(coax_v_u8){transcript_body,sizeof transcript_body});
    if(transcript_n<1||!xh_request_read((coax_string){(const char *)transcript_body,transcript_n},(coax_v_t_XhRequest){&transcript_request,1},(coax_v_u8){transcript_request_arena,sizeof transcript_request_arena})){api_error(q->fd,400,"Invalid transcript reader, capture, digest or offset.");return true;}
  }
  PGconn *db=api_db();if(!db){api_error(q->fd,503,"Task storage unavailable.");return true;}
  if(!api_limit(db,"environment-task",p->account,(int)ea_rate(),NULL)){api_error(q->fd,429,"Try again shortly.");return true;}
  const char *args[]={p->account,host};PGresult *connection=PQexecParams(db,
    "SELECT c.grant_id,c.epoch,c.relay,(SELECT count(*) FROM chat.environment_connections_v1 x WHERE x.host_pubkey=c.host_pubkey AND x.grant_id=c.grant_id) FROM chat.environment_connections_v1 c WHERE c.account=$1 AND c.host_pubkey=$2",2,NULL,args,NULL,NULL,0);
  if(!connection||PQresultStatus(connection)!=PGRES_TUPLES_OK){PQclear(connection);api_error(q->fd,503,"Connection storage unavailable.");return true;}
  bool write=post&&ta_write((coax_v_t_XtRequest){&control,1});
  int admission=(int)ta_admit(p->kind,p->level,es(p->account),PQntuples(connection)==1,PQntuples(connection)==1?atoll(PQgetvalue(connection,0,3)):0,post?control.method:es("ReadTask"),p->team,p->limited,p->frozen!=0);
  if(admission!=200){PQclear(connection);api_error(q->fd,admission,admission==404?"Environment not found.":admission==409?"This enrollment is not exclusive to this account.":"Task control unavailable for this account.");return true;}
  const char *key=getenv("OA_ENV_BROKER_KEY_FILE"),*program=getenv("OA_ENV_BROKER_PROGRAM");
  if(!key||!*key){PQclear(connection);api_error(q->fd,503,"Environment broker unavailable.");return true;}
  char file[]="/tmp/oa-task-control-XXXXXX";int fd=mkstemp(file);if(fd>=0)close(fd);
  unsigned char out[8192],second[47104],status_arena[2048],after_arena[2048];coax_t_XtStatus before={0},after={0};
  int status=503;const char *why="Task broker unavailable; reconcile an uncertain command before retrying.";int64_t n=-1;
  if(fd<0||!ea_payload_file(es(file),(coax_string){(const char *)read_body,read_n}))goto done;
  if(models){
    n=environment_task_proxy(es(host),es(PQgetvalue(connection,0,0)),es(PQgetvalue(connection,0,1)),es(PQgetvalue(connection,0,2)),es(key),es(file),es(program&&*program?program:"/usr/local/bin/environment-host"),(coax_v_u8){second,sizeof second});status=(int)ta_http(n);if(status!=200)goto done;
    coax_t_XoOffer offer={0};unsigned char offer_arena[16384];
    if(!xo_read((coax_string){(const char *)second,n},(coax_v_t_XoOffer){&offer,1},(coax_v_u8){offer_arena,sizeof offer_arena})||!xo_reply(offer_request,offer,es(host),(int64_t)api_now_ms())){status=502;why="Invalid or mismatched native model observation.";goto done;}
    env_reply(q->fd,second,(size_t)n);goto replied;
  }
  n=environment_task_proxy(es(host),es(PQgetvalue(connection,0,0)),es(PQgetvalue(connection,0,1)),es(PQgetvalue(connection,0,2)),es(key),es(file),es(program&&*program?program:"/usr/local/bin/environment-host"),(coax_v_u8){out,sizeof out});
  status=(int)ta_http(n);if(status!=200)goto done;
  if(!ta_status((coax_string){(const char *)out,n},es(host),es(task),es(""),(coax_v_t_XtStatus){&before,1},(coax_v_u8){status_arena,sizeof status_arena})){status=502;why="Invalid task status from the computer.";goto done;}
  char conversation[37]={0},turn[37]={0};memcpy(conversation,before.binding.conversation.data,(size_t)before.binding.conversation.count);memcpy(turn,before.binding.turn.data,(size_t)before.binding.turn.count);
  const char *owner_args[]={p->account,turn,conversation};PGresult *owner=PQexecParams(db,
    "SELECT 1 FROM chat.turns_v2 t JOIN chat.conversations_v2 c ON c.account=t.account AND c.id=t.conversation WHERE t.account=$1 AND t.id=$2 AND t.conversation=$3",3,NULL,owner_args,NULL,NULL,0);
  bool owned=owner&&PQresultStatus(owner)==PGRES_TUPLES_OK&&PQntuples(owner)==1;
  bool scoped=ta_scope(owned,es(host),before.binding.host,es(task),before.binding.task);bool owner_storage=owner&&PQresultStatus(owner)==PGRES_TUPLES_OK;PQclear(owner);
  if(!scoped){status=owner_storage?404:503;why=owner_storage?"Task not found.":"Task ownership storage unavailable.";goto done;}
  // Server-routed turns additionally pin their whole immutable binding and
  // enrollment. Older explicitly admitted tasks retain their existing scope.
  PGresult *association=PQexecParams(db,"SELECT record FROM chat.turn_tasks_v1 WHERE account=$1 AND turn=$2",2,NULL,owner_args,NULL,NULL,0);
  bool associated_storage=association&&PQresultStatus(association)==PGRES_TUPLES_OK;bool associated=associated_storage&&PQntuples(association)==1;
  if(!associated_storage){PQclear(association);status=503;why="Task association storage unavailable.";goto done;}
  if(associated){coax_t_CttTask retained={0};unsigned char retained_arena[20480],digest[64];bool valid=ctt_read(es(PQgetvalue(association,0,0)),(coax_v_t_CttTask){&retained,1},(coax_v_u8){retained_arena,sizeof retained_arena});coax_t_XtStatus expected={.binding=retained.binding};
    bool same=valid&&ctt_owned(retained,es(turn),es(conversation))&&ta_same_binding((coax_v_t_XtStatus){&expected,1},(coax_v_t_XtStatus){&before,1})&&ctn_enrollment(es(host),es(PQgetvalue(connection,0,0)),es(PQgetvalue(connection,0,1)),es(PQgetvalue(connection,0,2)),(coax_v_u8){digest,sizeof digest})&&ctn_scope(true,1,retained.enrollment,(coax_string){(const char *)digest,64});
    PQclear(association);if(!same){status=valid?404:503;why=valid?"Task not found.":"Saved task association is invalid.";goto done;}
  }else PQclear(association);
  if(transcript){
    if(!ea_payload_file(es(file),(coax_string){(const char *)transcript_body,transcript_n})){status=503;goto done;}
    n=environment_task_proxy(es(host),es(PQgetvalue(connection,0,0)),es(PQgetvalue(connection,0,1)),es(PQgetvalue(connection,0,2)),es(key),es(file),es(program&&*program?program:"/usr/local/bin/environment-host"),(coax_v_u8){second,sizeof second});
    status=(int)ta_http(n);if(status!=200)goto done;
    coax_t_XhPage page={0};unsigned char page_arena[20480];
    if(!xh_page_read((coax_string){(const char *)second,n},(coax_v_t_XhPage){&page,1},(coax_v_u8){page_arena,sizeof page_arena})||!xh_reply(transcript_request,page,es(host),(int64_t)api_now_ms())||!ta_same_binding((coax_v_t_XtStatus){&before,1},(coax_v_t_XtStatus){&page.capture.status,1})){status=502;why="Invalid or mismatched transcript capture from the computer.";goto done;}
    env_reply(q->fd,second,(size_t)n);goto replied;
  }
  if(!write){env_reply(q->fd,out,(size_t)n);goto replied;}
  if(!ea_payload_file(es(file),(coax_string){q->body,(int64_t)q->body_len})){status=503;goto done;}
  n=environment_task_proxy(es(host),es(PQgetvalue(connection,0,0)),es(PQgetvalue(connection,0,1)),es(PQgetvalue(connection,0,2)),es(key),es(file),es(program&&*program?program:"/usr/local/bin/environment-host"),(coax_v_u8){second,sizeof second});
  status=(int)ta_http(n);if(status!=200)goto done;
  if(!ta_status((coax_string){(const char *)second,n},es(host),es(task),control.command,(coax_v_t_XtStatus){&after,1},(coax_v_u8){after_arena,sizeof after_arena})||!ta_same_binding((coax_v_t_XtStatus){&before,1},(coax_v_t_XtStatus){&after,1})){status=502;why="Task control reply changed its binding; reconcile before retrying.";goto done;}
  PGresult *wake=PQexecParams(db,"UPDATE chat.turn_tasks_v1 SET poll_after_ms=0 WHERE account=$1 AND turn=$2",2,NULL,owner_args,NULL,NULL,0);if(wake&&PQresultStatus(wake)==PGRES_COMMAND_OK)(void)chat_wake();PQclear(wake);
  env_reply(q->fd,second,(size_t)n);goto replied;
 done: api_error(q->fd,status,why);
 replied: if(fd>=0)unlink(file);PQclear(connection);return true;
}
bool environment_route(const request *q) {
  if(environment_task_route(q))return true;
  bool workspace=!strcmp(q->path,"/api/chat/environment/workspace/v1");
  bool accounts=!strcmp(q->path,"/api/chat/environment/accounts/v1"),station=!strcmp(q->path,"/api/chat/environment/station/v1");
  bool list=!strcmp(q->path,"/api/chat/environments/v1"),remove_env=!strcmp(q->path,"/api/chat/environments/v1/remove"),call=!strcmp(q->path,"/api/chat/environment/v1");
  call=call || accounts || station || workspace;
  if(workspace && strcmp(q->method,"POST")){api_error(q->fd,405,"POST this.");return true;}
  if(remove_env && strcmp(q->method,"POST")){api_error(q->fd,405,"POST this.");return true;}
  if (!list && !remove_env && !call) return false;
  const auth_principal *p=auth_principal_of(q);
  if (!p || !ea_account(p->kind,es(p->account),es(p->account))) {api_error(q->fd,401,"Log in first.");return true;}
  bool post=!strcmp(q->method,"POST");
  if ((station && post) || (!post && strcmp(q->method,"GET"))) {api_error(q->fd,405,station ? "GET this":"GET or POST this");return true;}
  if (post && (list || remove_env) && !auth_same_site(q)) {api_error(q->fd,403,"Cross-site write refused.");return true;}
  PGconn *db=api_db();if (!db) {api_error(q->fd,503,"Connection storage unavailable.");return true;}
  if (!api_limit(db,"environment-call",p->account,(int)ea_rate(),NULL)) {api_error(q->fd,429,"Try again shortly.");return true;}
  if (post && list) {
    if (!ea_enroll_admit_c(p->kind,p->level>=1,es(p->account),p->limited,p->frozen!=0)) {api_error(q->fd,403,"This account cannot enroll computers.");return true;}
    coax_t_EaEnrollRequest *req=calloc(1,sizeof *req);
    if (!req) {api_error(q->fd,503,"Memory unavailable.");return true;}
    bool ok=ea_enroll_read_c((coax_string){q->body,(int64_t)q->body_len},(coax_v_t_EaEnrollRequest){req,1});
    if (!ok) {free(req);api_error(q->fd,400,"Invalid computer enrollment payload.");return true;}
    char host_s[65]={0},grant_s[65]={0},label_s[81]={0},relay_s[257]={0},os_s[17]={0},epoch_s[32]={0},now_s[32]={0};
    memcpy(host_s,req->host.e,64);memcpy(grant_s,req->grant.e,64);
    memcpy(label_s,req->label.e,(size_t)req->nl);memcpy(relay_s,req->relay.e,(size_t)req->nrelay);
    memcpy(os_s,req->os.e,(size_t)req->nos);
    snprintf(epoch_s,sizeof epoch_s,"%lld",(long long)req->epoch);
    long long now_ms=(long long)api_now_ms();snprintf(now_s,sizeof now_s,"%lld",now_ms);
    unsigned char *ws_json=calloc(1,16385),*out=calloc(1,32768);
    if (!ws_json || !out) {free(ws_json);free(out);free(req);api_error(q->fd,503,"Memory unavailable.");return true;}
    int64_t wn=ea_enroll_workspaces_write_c((coax_v_t_EaWorkspaceEntry){req->workspaces.e,16},req->nws,(coax_v_u8){ws_json,16384});
    if (wn<2) {free(ws_json);free(out);free(req);api_error(q->fd,400,"Workspace list too large.");return true;}
    ws_json[wn]=0;
    const char *chk_args[]={host_s,grant_s,p->account};
    PGresult *chk=PQexecParams(db,"SELECT 1 FROM chat.environment_connections_v1 WHERE host_pubkey=$1 AND grant_id=$2 AND account<>$3 LIMIT 1",3,NULL,chk_args,NULL,NULL,0);
    if (!chk || PQresultStatus(chk)!=PGRES_TUPLES_OK) {PQclear(chk);free(ws_json);free(out);free(req);api_error(q->fd,503,"Connection storage unavailable.");return true;}
    if (PQntuples(chk)>0) {PQclear(chk);free(ws_json);free(out);free(req);api_error(q->fd,409,"This computer grant is already enrolled on another account.");return true;}
    PQclear(chk);
    const char *up_args[]={p->account,host_s,label_s,grant_s,epoch_s,relay_s,os_s,(const char *)ws_json,now_s};
    PGresult *up=PQexecParams(db,
      "INSERT INTO chat.environment_connections_v1(account,host_pubkey,display_name,grant_id,epoch,relay,os,workspaces_json,last_seen_ms) "
      "VALUES($1,$2,$3,$4,$5::bigint,$6,$7,$8,$9::bigint) "
      "ON CONFLICT(account,host_pubkey) DO UPDATE SET display_name=EXCLUDED.display_name,grant_id=EXCLUDED.grant_id,epoch=EXCLUDED.epoch,relay=EXCLUDED.relay,os=EXCLUDED.os,workspaces_json=EXCLUDED.workspaces_json,last_seen_ms=EXCLUDED.last_seen_ms",
      9,NULL,up_args,NULL,NULL,0);
    bool saved=up && PQresultStatus(up)==PGRES_COMMAND_OK;PQclear(up);
    int64_t rn=saved?ea_enroll_reply_write_c((coax_v_t_EaEnrollRequest){req,1},now_ms,(coax_v_u8){out,32768}):-1;
    free(ws_json);free(req);
    if (!saved || rn<1) {free(out);api_error(q->fd,503,"Computer enrollment could not be saved.");return true;}
    env_reply(q->fd,out,(size_t)rn);free(out);return true;
  }
  if (remove_env) {
    if (!ea_enroll_admit_c(p->kind,p->level>=1,es(p->account),p->limited,p->frozen!=0)) {api_error(q->fd,403,"This account cannot change enrolled computers.");return true;}
    unsigned char host_b[65]={0};
    if (!ea_enroll_remove_read_c((coax_string){q->body,(int64_t)q->body_len},(coax_v_u8){host_b,64})) {api_error(q->fd,400,"Invalid computer removal payload.");return true;}
    const char *del_args[]={p->account,(const char *)host_b};
    PGresult *db1=PQexecParams(db,"DELETE FROM projects.environment_bindings_v1 WHERE account=$1 AND host_pubkey=$2",2,NULL,del_args,NULL,NULL,0);
    PQclear(db1);
    PGresult *dr=PQexecParams(db,"DELETE FROM chat.environment_connections_v1 WHERE account=$1 AND host_pubkey=$2 RETURNING host_pubkey",2,NULL,del_args,NULL,NULL,0);
    bool ok=dr && PQresultStatus(dr)==PGRES_TUPLES_OK;int gone=ok?PQntuples(dr):0;PQclear(dr);
    if (!ok) {api_error(q->fd,503,"Connection storage unavailable.");return true;}
    if (gone<1) {api_error(q->fd,404,"Environment not found.");return true;}
    char rep[160];int rn=snprintf(rep,sizeof rep,"{\"schema\":\"openagents.environment.enroll.v1alpha1\",\"removed\":true,\"host_pubkey\":\"%s\"}",(const char *)host_b);
    env_reply(q->fd,rep,(size_t)rn);return true;
  }
  char host[65]={0};if (call) api_query(q,"host",host,sizeof host);
  const char *args[]={p->account,host};
  PGresult *r=PQexecParams(db,list ?
    "SELECT host_pubkey,display_name,grant_id,epoch,relay,os,workspaces_json,last_seen_ms FROM chat.environment_connections_v1 WHERE account=$1 ORDER BY display_name LIMIT 64":
    "SELECT host_pubkey,display_name,grant_id,epoch,relay FROM chat.environment_connections_v1 WHERE account=$1 AND host_pubkey=$2",list ? 1:2,NULL,args,NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK) {PQclear(r);api_error(q->fd,503,"Connection storage unavailable.");return true;}
  if (list) {
    char as_q[16]={0},detail_q[8]={0};api_query(q,"as",as_q,sizeof as_q);api_query(q,"detail",detail_q,sizeof detail_q);
    long long now_ms=(long long)api_now_ms();
    if (!strcmp(as_q,"lines")) {
      unsigned char *out=calloc(1,65536),*one=calloc(1,8192);
      if (!out || !one) {free(out);free(one);PQclear(r);api_error(q->fd,503,"Memory unavailable.");return true;}
      size_t used=0;int nrows=PQntuples(r);
      for (int i=0;i<nrows;i++) {
        int64_t n=ea_catalog_lines_entry_write_c(es(PQgetvalue(r,i,0)),es(PQgetvalue(r,i,1)),es(PQgetvalue(r,i,5)),atoll(PQgetvalue(r,i,7)),now_ms,es(PQgetvalue(r,i,6)),(coax_v_u8){one,8192});
        if (n>0 && used+(size_t)n<65536) {memcpy(out+used,one,(size_t)n);used+=(size_t)n;}
      }
      PQclear(r);free(one);
      api_respond(q->fd,200,"text/plain; charset=utf-8","Cache-Control: no-store\r\nVary: Cookie, Authorization\r\n",out,used);
      free(out);return true;
    }
    if (!strcmp(detail_q,"1")) {
      coax_string entries[64];char *owned[64]={0};int count=0;bool ok=true;
      unsigned char *one=calloc(1,24576),*out=calloc(1,262144);
      if (!one || !out) ok=false;
      int nrows=PQntuples(r);
      for (int i=0;ok && i<nrows && i<64;i++) {
        int64_t n=ea_catalog_v2_entry_write_c(es(PQgetvalue(r,i,0)),es(PQgetvalue(r,i,1)),es(PQgetvalue(r,i,5)),es(PQgetvalue(r,i,4)),atoll(PQgetvalue(r,i,7)),now_ms,es(PQgetvalue(r,i,6)),(coax_v_u8){one,24575});
        if (n<1) {ok=false;break;}
        one[n]=0;owned[count]=strdup((const char *)one);if (!owned[count]) {ok=false;break;}
        entries[count]=es(owned[count]);count++;
      }
      int64_t total=ok?ea_catalog_v2_wrap_c((coax_v_string){entries,count},(coax_v_u8){out,262144}):-1;
      for (int i=0;i<count;i++) free(owned[i]);
      free(one);PQclear(r);
      if (total<1) api_error(q->fd,503,"Connection list unavailable.");else env_reply(q->fd,out,(size_t)total);
      free(out);return true;
    }
    coax_t_EaCatalog *c=calloc(1,sizeof *c);unsigned char out[16384];
    if (!c) {PQclear(r);api_error(q->fd,503,"Memory unavailable.");return true;}
    c->n=PQntuples(r);for (int i=0;i<c->n;i++) {memcpy(c->entries.e[i].host.e,PQgetvalue(r,i,0),64);c->entries.e[i].nl=(int64_t)strlen(PQgetvalue(r,i,1));memcpy(c->entries.e[i].label.e,PQgetvalue(r,i,1),(size_t)c->entries.e[i].nl);}
    int64_t n=ea_catalog_write((coax_v_t_EaCatalog){c,1},(coax_v_u8){out,sizeof out});free(c);PQclear(r);
    if (n<1) api_error(q->fd,503,"Connection list unavailable.");else env_reply(q->fd,out,(size_t)n);return true;
  }
  if (PQntuples(r)!=1) {PQclear(r);api_error(q->fd,404,"Environment not found.");return true;}
  const char *key=getenv("OA_ENV_BROKER_KEY_FILE"),*program=getenv("OA_ENV_BROKER_PROGRAM");
  if (station) {
    // the computer's own report (coder/tui/host-station.coax); the broker builds the params
    if (!key || !*key) {PQclear(r);api_error(q->fd,503,"Environment broker unavailable.");return true;}
    char refresh[8];api_query(q,"refresh",refresh,sizeof refresh);
    unsigned char *out=malloc(65536);int64_t n= -1;
    if (out) n=environment_station_proxy(es(PQgetvalue(r,0,0)),es(PQgetvalue(r,0,2)),es(PQgetvalue(r,0,3)),es(PQgetvalue(r,0,4)),es(key),!strcmp(refresh,"1"),es(program && *program ? program:"/usr/local/bin/environment-host"),(coax_v_u8){out,65536});
    PQclear(r);
    if (n<1) {api_log("environment station unavailable: result=%lld",(long long)n);api_error(q->fd,503,"That computer did not report.");}else env_reply(q->fd,out,(size_t)n);
    free(out);return true;
  }
  unsigned char read_body[24576];const char *payload=q->body;size_t payload_len=q->body_len;
  if (!strcmp(q->method,"GET")) {
    char method[33],id[129],before[32];api_query(q,"method",method,sizeof method);api_query(q,"session",id,sizeof id);api_query(q,"before",before,sizeof before);
    char refresh[8];api_query(q,"refresh",refresh,sizeof refresh);
    int64_t made=accounts ? pa_list_body(!strcmp(refresh,"1"),(coax_v_u8){read_body,sizeof read_body}):ea_read_body(es(method),es(id),es(before),(coax_v_u8){read_body,sizeof read_body});
    if (made<1) {PQclear(r);api_error(q->fd,400,"Unsupported read method.");return true;}payload=(const char *)read_body;payload_len=(size_t)made;
  }
  bool valid=false;
  if(workspace){coax_t_WsRequest *body=calloc(1,sizeof *body);valid=body && ws_read((coax_string){payload,(int64_t)payload_len},(coax_v_t_WsRequest){body,1});free(body);}
  else if (accounts) {
    coax_t_PaRequest *body=calloc(1,sizeof *body);
    valid=body && pa_request_read((coax_string){payload,(int64_t)payload_len},(coax_v_t_PaRequest){body,1});
    if (body) {volatile unsigned char *secret=(volatile unsigned char *)body;for (size_t i=0;i<sizeof *body;i++) secret[i]=0;free(body);}
  } else {
    coax_t_EaRequest *body=calloc(1,sizeof *body);valid=body && ea_read((coax_string){payload,(int64_t)payload_len},(coax_v_t_EaRequest){body,1});free(body);
  }
  if (!valid) {PQclear(r);api_error(q->fd,400,"Invalid environment request.");return true;}
  if (!key || !*key) {PQclear(r);api_error(q->fd,503,"Environment broker unavailable.");return true;}

  char path[]="/tmp/oa-environment-XXXXXX";int f=mkstemp(path);if (f>=0) close(f);
  bool saved=f>=0 && ea_payload_file(es(path),(coax_string){payload,(int64_t)payload_len});
  unsigned char *out=malloc(1048576);int64_t n= -1;
  if (saved && out) n=(workspace ? environment_workspace_proxy:accounts ? environment_accounts_proxy:environment_proxy)(es(PQgetvalue(r,0,0)),es(PQgetvalue(r,0,2)),es(PQgetvalue(r,0,3)),es(PQgetvalue(r,0,4)),es(key),es(path),es(program && *program ? program:"/usr/local/bin/environment-host"),(coax_v_u8){out,1048576});
  unlink(path);PQclear(r);
  if (n<1) {api_log("environment broker unavailable: saved=%d result=%lld",saved,(long long)n);api_error(q->fd,503,"Computer unavailable. An uncertain command is never resent automatically.");}else env_reply(q->fd,out,(size_t)n);
  free(out);return true;
}
