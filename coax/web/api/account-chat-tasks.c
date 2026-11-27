// Platform effects for the durable chat's existing native HOST execution owner.
// Included by account-chat-turns.c; policy, identity and JSON stay in Coax.
#include <signal.h>
#include <unistd.h>
typedef struct {char host[65],grant[65],epoch[17],relay[2049],name[81],digest[65];} native_connection;
typedef struct {
 turn_model m;char *raw,*record,*prior,*stop_record;int phase;bool stop;
 char file[64];native_connection connection;double deadline;
 coax_t_CttTask task;unsigned char arena[20480];
} native_turn;
static bool native_guard(native_turn *n,bool write){
 PGconn *db=model_db(&n->m);if(!db||!exec_sql(db,"BEGIN",0,NULL))return false;
 PGresult *r=turn_row(db,n->m.account,n->m.turn,true);bool ok=model_matches(&n->m,r)&&number(r,0,3)==RUNNING&&nullable(r,0,26)>=(long long)api_now_ms();
 if(ok){n->stop=PQgetvalue(r,0,4)[0]=='t';if(write){const char *v[]={n->m.account,PQgetvalue(r,0,30)};PGresult *a=PQexecParams(db,"SELECT kind,team,frozen_at IS NOT NULL,deleted_at IS NOT NULL,($2='' OR EXISTS(SELECT 1 FROM projects.private_v1 WHERE account=$1 AND id=$2)) FROM auth.accounts WHERE id=$1",2,NULL,v,NULL,NULL,0);
   ok=tuples(a)&&PQntuples(a)==1&&ctw_grant(act_funded_turn(!strcmp(PQgetvalue(a,0,0),"person")?2:0,ts(n->m.account),PQgetvalue(a,0,1)[0]=='t',ts(PQgetvalue(r,0,31))),PQgetvalue(a,0,2)[0]=='t',PQgetvalue(a,0,3)[0]=='t',PQgetvalue(a,0,4)[0]=='t');PQclear(a);}}
 if(ok){char now[32],until[32];long long ms=(long long)api_now_ms();num(now,ms);num(until,ctw_claim_end(ms));const char *v[]={n->m.account,n->m.turn,n->m.dispatch,now,until};ok=exec_sql(db,"UPDATE chat.turns_v2 SET heartbeat_ms=$4::bigint,claim_until_ms=$5::bigint WHERE account=$1 AND id=$2 AND dispatch=$3",5,v);}
 PQclear(r);if(ok&&exec_sql(db,"COMMIT",0,NULL))return true;exec_sql(db,"ROLLBACK",0,NULL);return false;
}
static bool native_connection_read(native_connection *c,PGresult *r,int i){
 return tuples(r)&&copy_string(c->host,sizeof c->host,ts(PQgetvalue(r,i,0)))&&copy_string(c->grant,sizeof c->grant,ts(PQgetvalue(r,i,1)))&&copy_string(c->epoch,sizeof c->epoch,ts(PQgetvalue(r,i,2)))&&copy_string(c->relay,sizeof c->relay,ts(PQgetvalue(r,i,3)))&&copy_string(c->name,sizeof c->name,ts(PQgetvalue(r,i,4)))&&ctn_enrollment(ts(c->host),ts(c->grant),ts(c->epoch),ts(c->relay),tb(c->digest,64));
}
static bool native_scope(native_turn *n,native_connection *c){
 PGconn *db=model_db(&n->m);if(!db)return false;const char *v[]={n->m.account,c->host};PGresult *r=PQexecParams(db,"SELECT host_pubkey,grant_id,epoch,relay,display_name,(SELECT count(*) FROM chat.environment_connections_v1 x WHERE x.host_pubkey=c.host_pubkey AND x.grant_id=c.grant_id) FROM chat.environment_connections_v1 c WHERE account=$1 AND host_pubkey=$2",2,NULL,v,NULL,NULL,0);
 native_connection current={0};bool ok=tuples(r)&&PQntuples(r)==1&&native_connection_read(&current,r,0)&&ctn_scope(true,number(r,0,5),ts(c->digest),ts(current.digest));PQclear(r);return ok;
}
static int64_t native_exchange(native_turn *n,coax_string body,unsigned char *out,size_t cap,bool write,bool accounts){
 if(api_now_ms()>=n->deadline){return -3000;}if(!native_guard(n,write)||!native_scope(n,&n->connection))return -2004;if(!ctn_effect(write,n->stop))return -2008;
 const char *key=getenv("OA_ENV_BROKER_KEY_FILE"),*program=getenv("OA_ENV_BROKER_PROGRAM");if(!key||!*key||!ea_payload_file(ts(n->file),body))return -5000;
 native_connection *c=&n->connection;return (accounts?environment_accounts_proxy:environment_task_proxy)(ts(c->host),ts(c->grant),ts(c->epoch),ts(c->relay),ts(key),ts(n->file),ts(program&&*program?program:"/usr/local/bin/environment-host"),tb(out,cap));
}
static int64_t native_rpc(native_turn *n,coax_string body,unsigned char *out,size_t cap,bool write){
 return native_exchange(n,body,out,cap,write,false);
}
static long native_decide(coax_string body,char **answer,size_t *count){
 const char *key=getenv("OA_GEMINI_DECISIONS_KEY"),*url=getenv("OA_ROUTER_DECISIONS_URL");*answer=NULL;*count=0;if(!key||!*key||strlen(key)>256)return 0;
 char header[300];snprintf(header,sizeof header,"x-goog-api-key: %s",key);long status=mesh_post_json(url&&*url?url:"https://generativelanguage.googleapis.com/v1beta/decisions",body.data,(size_t)body.count,header,8000,answer,count);memset(header,0,sizeof header);return status;
}
static void native_finish(native_turn *n,int outcome,const char *error){
 if(outcome==3&&!n->stop){if(!model_emit(&n->m,STOP,ts(""),ts(""),ts(""),false))return;n->stop=true;}
 int kind=outcome==1?END:outcome==2?ERROR:outcome==3?CANCEL:LOST;
 (void)model_emit(&n->m,kind,ts(""),ts(outcome==1?"end_turn":outcome==2?"error":outcome==3?"cancelled":"unknown"),ts(error),false);
}
static bool native_load(native_turn *n){
 const char *v[]={n->m.account,n->m.turn};PGresult *r=PQexecParams(model_db(&n->m),"SELECT t.native_input_json,x.record,x.phase,coalesce(x.status_json,''),coalesce(x.stop_json,''),t.conversation FROM chat.turns_v2 t LEFT JOIN chat.turn_tasks_v1 x ON x.account=t.account AND x.turn=t.id WHERE t.account=$1 AND t.id=$2",2,NULL,v,NULL,NULL,0);
 bool ok=tuples(r)&&PQntuples(r)==1&&!PQgetisnull(r,0,0);if(ok){n->raw=strdup(PQgetvalue(r,0,0));ok=n->raw!=NULL;
  if(!PQgetisnull(r,0,1)){n->record=strdup(PQgetvalue(r,0,1));n->phase=(int)number(r,0,2);n->prior=strdup(PQgetvalue(r,0,3));n->stop_record=strdup(PQgetvalue(r,0,4));ok=n->record&&n->prior&&n->stop_record&&ctt_read(ts(n->record),(coax_v_t_CttTask){&n->task,1},tb(n->arena,sizeof n->arena))&&ctt_owned(n->task,ts(n->m.turn),ts(PQgetvalue(r,0,5)));}}
 PQclear(r);return ok;
}
static bool native_save_association(native_turn *n,const char *record){
 PGconn *db=model_db(&n->m);if(!native_guard(n,true)||n->stop||!native_scope(n,&n->connection)||!exec_sql(db,"BEGIN",0,NULL))return false;
 PGresult *r=turn_row(db,n->m.account,n->m.turn,true);bool ok=model_matches(&n->m,r)&&number(r,0,3)==RUNNING&&PQgetvalue(r,0,4)[0]!='t';
 char ms[32];num(ms,(long long)api_now_ms());const char *v[]={n->m.account,n->m.turn,record,ms};PGresult *saved=ok?PQexecParams(db,"INSERT INTO chat.turn_tasks_v1(account,turn,record,created_ms,updated_ms) VALUES($1,$2,$3,$4::bigint,$4::bigint) ON CONFLICT DO NOTHING RETURNING turn",4,NULL,v,NULL,NULL,0):NULL;ok=ok&&tuples(saved)&&PQntuples(saved)==1;
 coax_t_CttTask task;unsigned char arena[20480];if(ok)ok=ctt_read(ts(record),(coax_v_t_CttTask){&task,1},tb(arena,sizeof arena));
 if(ok){char model[129],effort[17],provider[16];copy_string(model,sizeof model,task.binding.model);copy_string(effort,sizeof effort,task.binding.effort);copy_string(provider,sizeof provider,ctn_provider(task.binding.engine));const char *p[]={n->m.account,n->m.turn,model,provider,effort};ok=exec_sql(db,"UPDATE chat.turns_v2 SET model=$3,provider=$4,effort=$5 WHERE account=$1 AND id=$2",5,p);}
 PQclear(saved);PQclear(r);if(ok&&exec_sql(db,"COMMIT",0,NULL))return true;exec_sql(db,"ROLLBACK",0,NULL);return false;
}
// Returns zero for ordinary hosted chat, one after retaining a native binding,
// minus one for a refused preflight. No native task exists before it returns.
static int native_preflight(native_turn *n,const char *conversation){
 coax_t_CtnRouter *h=calloc(1,sizeof *h);coax_t_CtnIntent *intent=calloc(1,sizeof *intent);coax_t_CtnSelect *select=calloc(1,sizeof *select);
 unsigned char *body=malloc(262145),*out=malloc(98305);native_connection connections[16]={0};int map[16]={0},result=-1;
 if(!h||!intent||!select||!body||!out||!ctn_init((coax_v_t_CtnRouter){h,1},ts(n->raw),(int64_t)api_now_ms()))goto done;
 coax_string text=h->q.e[0].text,summary=h->q.e[0].summary;int64_t size=ctn_intent_begin((coax_v_t_CtnIntent){intent,1},text,summary,tb(body,262144));if(size<1)goto done;
 char *answer=NULL;size_t count=0;long status=native_decide(tn(body,size),&answer,&count);int work=(int)ctn_intent_finish((coax_v_t_CtnIntent){intent,1},status,tn(answer?answer:"",count));free(answer);if(work==0){result=0;goto done;}if(work<0)goto done;
 char pinned_host[65]={0};
 {const char *wv[]={n->m.account,n->m.turn};PGresult *wr=PQexecParams(model_db(&n->m),"SELECT coalesce(workspace,'') FROM chat.turns_v2 WHERE account=$1 AND id=$2",2,NULL,wv,NULL,NULL,0);
  if(tuples(wr)&&PQntuples(wr)==1){const char *ws=PQgetvalue(wr,0,0);if(!strncmp(ws,"host:",5)&&strlen(ws)==69)snprintf(pinned_host,sizeof pinned_host,"%s",ws+5);}PQclear(wr);}
 const char *args[]={n->m.account,pinned_host};PGresult *r=PQexecParams(model_db(&n->m),"SELECT host_pubkey,grant_id,epoch,relay,display_name FROM chat.environment_connections_v1 WHERE account=$1 AND ($2='' OR host_pubkey=$2) ORDER BY host_pubkey LIMIT 17",2,NULL,args,NULL,NULL,0);
 if(!tuples(r)||PQntuples(r)>16){PQclear(r);goto done;}int total=PQntuples(r);for(int i=0;i<total;i++)if(!native_connection_read(&connections[i],r,i)){PQclear(r);goto done;}PQclear(r);
 const char *key=getenv("OA_ENV_BROKER_KEY_FILE"),*program=getenv("OA_ENV_BROKER_PROGRAM");if(!key||!*key)goto done;
 double deadline=api_now_ms()+180000;
 for(int i=0;i<total&&api_now_ms()<deadline;i++){
  native_connection *c=&connections[i];if(!native_guard(n,true)||n->stop)goto done;if(!native_scope(n,c))continue;
  int64_t got=environment_station_proxy(ts(c->host),ts(c->grant),ts(c->epoch),ts(c->relay),ts(key),true,ts(program&&*program?program:"/usr/local/bin/environment-host"),tb(out,65536));
  int row=(int)h->q.e[0].n;if(got>0&&ctn_report_add((coax_v_t_CtnRouter){h,1},ts(c->host),ts(c->name),tn(out,got)))map[row]=i;
 }
 size=ctn_route_begin((coax_v_t_CtnRouter){h,1},(int64_t)api_now_ms(),tb(body,262144));if(size<1)goto done;
 status=native_decide(tn(body,size),&answer,&count);int row=(int)ctn_route_finish((coax_v_t_CtnRouter){h,1},status,tn(answer?answer:"",count));free(answer);if(row<0)goto done;n->connection=connections[map[row]];
 char provider_account[33]="system";
 if(h->d.e[0].engine.count==6&&!memcmp(h->d.e[0].engine.data,"claude",6)){
  size=pa_list_body(true,tb(body,262144));if(size<1)goto done;bool ready=false;
  for(int attempt=0;attempt<32&&api_now_ms()<deadline;attempt++){
   int64_t got=native_exchange(n,tn(body,size),out,98304,true,true);if(got<1)goto done;
   int64_t na=ctn_account_pick(tn(out,got),ts(n->connection.host),(int64_t)api_now_ms(),tb(provider_account,32));
   if(na>0&&na<=32){provider_account[na]=0;ready=true;break;}
   if(!ctn_account_pending(tn(out,got),ts(n->connection.host)))goto done;
  }
  if(!ready)goto done;
 }
 char probe[37],command[37];if(!api_object_id(probe,"cmd_")||!api_object_id(command,"cmd_"))goto done;
 coax_t_XoRequest oq={.probe=tn(probe+4,32),.engine=h->d.e[0].engine,.account=ts(provider_account)};size=xo_request_write(oq,tb(body,262144));if(size<1)goto done;
 for(int attempt=0;attempt<32&&api_now_ms()<deadline;attempt++){
  int64_t got=native_rpc(n,tn(body,size),out,98304,true);coax_t_XoOffer offer;unsigned char arena[16384];
  if(got<1||!xo_read(tn(out,got),(coax_v_t_XoOffer){&offer,1},tb(arena,sizeof arena))||!xo_reply(oq,offer,ts(n->connection.host),(int64_t)api_now_ms()))goto done;
  if(offer.state==0){continue;}if(offer.state!=1)goto done;
  int64_t choice=ctn_select_begin((coax_v_t_CtnSelect){select,1},tn(out,got),text,h->d.e[0].model,h->d.e[0].effort,tb(body,262144));if(choice<0)goto done;
  bool chosen=choice==0;if(choice>0){status=native_decide(tn(body,choice),&answer,&count);chosen=ctn_select_finish((coax_v_t_CtnSelect){select,1},status,tn(answer?answer:"",count));free(answer);}if(!chosen)goto done;
  size=ctn_bind((coax_v_t_CtnRouter){h,1},(coax_v_t_CtnSelect){select,1},row,ts(n->m.turn),ts(conversation),tn(command+4,32),ts(n->connection.digest),ts(n->raw),(int64_t)api_now_ms(),tb(out,98304));if(size<1)goto done;out[size]=0;
  if(!native_save_association(n,(char *)out)){goto done;}n->record=strdup((char *)out);n->prior=strdup("");n->stop_record=strdup("");n->phase=0;
  if(n->record&&n->prior&&n->stop_record&&ctt_read(ts(n->record),(coax_v_t_CttTask){&n->task,1},tb(n->arena,sizeof n->arena))){result=1;}goto done;
 }
 done:free(h);free(intent);free(select);free(body);free(out);return result;
}
static bool native_stop_store(PGconn *db,native_turn *n,const char *record,const char *now){
 if(!record||!*record)return true;
 coax_t_XtRequest q;unsigned char arena[2048];char command[33]={0};
 if(!xt_request_read(ts(record),(coax_v_t_XtRequest){&q,1},tb(arena,sizeof arena))||!xc_match(q,ts("stop"),n->task.binding.task,0,ts(""),ts(""))||!copy_string(command,sizeof command,q.command))return false;
 const char *v[]={n->m.account,n->m.turn,command,record,now};
 PGresult *r=PQexecParams(db,"INSERT INTO chat.task_stop_commands_v1(account,turn,command,record,created_ms) VALUES($1,$2,$3,$4,$5::bigint) ON CONFLICT(account,turn,command) DO UPDATE SET record=chat.task_stop_commands_v1.record WHERE chat.task_stop_commands_v1.record=EXCLUDED.record RETURNING command",5,NULL,v,NULL,NULL,0);
 bool ok=tuples(r)&&PQntuples(r)==1;PQclear(r);return ok;
}
static bool native_store(native_turn *n,int phase,const char *status,const char *stop){
 PGconn *db=model_db(&n->m);if(!native_guard(n,false)||!exec_sql(db,"BEGIN",0,NULL))return false;PGresult *r=turn_row(db,n->m.account,n->m.turn,true);
 bool ok=model_matches(&n->m,r)&&number(r,0,3)==RUNNING;if(ok){n->stop=PQgetvalue(r,0,4)[0]=='t';}if(ok&&phase==1&&n->phase==0&&n->stop)ok=false;
 char p[32],ms[32];num(p,phase);num(ms,(long long)api_now_ms());const char *v[]={n->m.account,n->m.turn,p,status,stop,ms};if(ok)ok=native_stop_store(db,n,n->stop_record,ms)&&native_stop_store(db,n,stop,ms)&&exec_sql(db,"UPDATE chat.turn_tasks_v1 SET phase=$3::int,status_json=nullif($4,''),stop_json=nullif($5,''),updated_ms=$6::bigint WHERE account=$1 AND turn=$2",6,v);
 char *next_status=ok?strdup(status):NULL,*next_stop=ok?strdup(stop):NULL;ok=ok&&next_status&&next_stop;
 PQclear(r);if(ok&&exec_sql(db,"COMMIT",0,NULL)){n->phase=phase;free(n->prior);free(n->stop_record);n->prior=next_status;n->stop_record=next_stop;return true;}free(next_status);free(next_stop);exec_sql(db,"ROLLBACK",0,NULL);return false;
}
static bool native_observe(native_turn *n,const unsigned char *raw,int64_t size,coax_string command,coax_t_XtStatus *status){
 unsigned char canonical[8193],arena[2048];int64_t count=ctn_status(n->task.binding,tn(raw,size),command,ts(n->prior),(coax_v_t_XtStatus){status,1},tb(arena,sizeof arena),tb(canonical,8192));if(count<1)return false;canonical[count]=0;
 // Only scalars survive this call; strings borrow raw/arena until normalized
 // status is retained. Re-read into caller-owned storage before using strings.
 return native_store(n,2,(char *)canonical,n->stop_record);
}
static bool native_capture_reset(native_turn *n,const char *request){
 PGconn *db=model_db(&n->m);if(!native_guard(n,false)||!exec_sql(db,"BEGIN",0,NULL))return false;
 PGresult *r=turn_row(db,n->m.account,n->m.turn,true);const char *v[]={n->m.account,n->m.turn,request};
 bool ok=model_matches(&n->m,r)&&number(r,0,3)==RUNNING&&exec_sql(db,"DELETE FROM chat.task_capture_pages_v1 WHERE account=$1 AND turn=$2",2,v)&&exec_sql(db,"UPDATE chat.turn_tasks_v1 SET capture_json=$3 WHERE account=$1 AND turn=$2",3,v);
 PQclear(r);if(ok&&exec_sql(db,"COMMIT",0,NULL))return true;exec_sql(db,"ROLLBACK",0,NULL);return false;
}
static bool native_capture_store(native_turn *n,int page,const unsigned char *raw,int64_t size,long long accepted){
 PGconn *db=model_db(&n->m);if(!xhr_page_room(page)||!native_guard(n,false)||!exec_sql(db,"BEGIN",0,NULL))return false;
 PGresult *r=turn_row(db,n->m.account,n->m.turn,true);char ordinal[32],ms[32];num(ordinal,page);num(ms,accepted);char *record=strndup((const char *)raw,(size_t)size);
 const char *v[]={n->m.account,n->m.turn,ordinal,record,ms};bool ok=record&&model_matches(&n->m,r)&&number(r,0,3)==RUNNING&&exec_sql(db,"INSERT INTO chat.task_capture_pages_v1(account,turn,page,record,accepted_ms) VALUES($1,$2,$3::int,$4,$5::bigint)",5,v);
 PQclear(r);free(record);if(ok&&exec_sql(db,"COMMIT",0,NULL))return true;exec_sql(db,"ROLLBACK",0,NULL);return false;
}
static bool native_capture(native_turn *n){
 coax_t_CtnCapture *c=calloc(1,sizeof *c);unsigned char *data=malloc(4194304),*pool=malloc(4194304),*decode=malloc(4194304),*text=malloc(131073),*delta=malloc(131073);bool ok=false;
 unsigned char request[1025],reply_bytes[47104],page_arena[20480];char read[37];int pages=0;
 if(!c||!data||!pool||!decode||!text||!delta||!native_guard(n,false)||!native_scope(n,&n->connection))goto done;
 const char *v[]={n->m.account,n->m.turn};PGresult *r=PQexecParams(model_db(&n->m),"SELECT coalesce(capture_json,'') FROM chat.turn_tasks_v1 WHERE account=$1 AND turn=$2",2,NULL,v,NULL,NULL,0);
 bool loaded=tuples(r)&&PQntuples(r)==1;int64_t initial=loaded?(int64_t)strlen(PQgetvalue(r,0,0)):-1;
 if(initial>0)loaded=xhr_begin((coax_v_t_CtnCapture){c,1},n->task.binding,ts(PQgetvalue(r,0,0)),tb(data,4194304));PQclear(r);if(!loaded)goto reset;
 if(initial==0){
  if(!api_object_id(read,"cmd_"))goto done;coax_t_XhRequest q={.task=n->task.binding.task,.reader=n->task.binding.task,.read=tn(read+4,32)};
  int64_t size=xh_request_write(q,tb(request,sizeof request-1));if(size<1)goto done;request[size]=0;
  if(!native_capture_reset(n,(char *)request)||!xhr_begin((coax_v_t_CtnCapture){c,1},n->task.binding,tn(request,size),tb(data,4194304)))goto done;
 }else {
  r=PQexecParams(model_db(&n->m),"SELECT page,record,accepted_ms FROM chat.task_capture_pages_v1 WHERE account=$1 AND turn=$2 ORDER BY page LIMIT 513",2,NULL,v,NULL,NULL,0);
  loaded=tuples(r);if(loaded){pages=PQntuples(r);for(int i=0;i<pages;i++)if(!xhr_page_room(i)||number(r,i,0)!=i||!xhr_restore((coax_v_t_CtnCapture){c,1},n->task.binding,ts(PQgetvalue(r,i,1)),number(r,i,2),(int64_t)api_now_ms(),tb(page_arena,sizeof page_arena))){loaded=false;break;}}
  PQclear(r);if(!loaded)goto reset;
 }
 if(!c->client.e[0].done&&!xhr_continuable((coax_v_t_CtnCapture){c,1},(int64_t)api_now_ms()))goto reset;
 for(int taken=0;!c->client.e[0].done&&xhr_claim_room(taken);taken++,pages++){
  if(!xhr_page_room(pages))goto reset;int64_t size=ctn_capture_request((coax_v_t_CtnCapture){c,1},tb(request,sizeof request-1));int64_t got=size>0?native_rpc(n,tn(request,size),reply_bytes,sizeof reply_bytes,false):-1;long long accepted=(long long)api_now_ms();
  if(got== -2005)goto reset;if(got<1)goto done;
  if(!ctn_capture_page((coax_v_t_CtnCapture){c,1},n->task.binding,tn(reply_bytes,got),accepted,tb(page_arena,sizeof page_arena)))goto reset;
  if(!native_capture_store(n,pages,reply_bytes,got,accepted))goto done;
 }
 if(!c->client.e[0].done)goto done;
 if(!xhr_publishable((coax_v_t_CtnCapture){c,1},n->task.binding,ts(n->prior)))goto reset;
 int64_t size=ctn_capture_text((coax_v_t_CtnCapture){c,1},tb(pool,4194304),tb(decode,4194304),tb(text,131072));if(size== -2){native_finish(n,2,"Native reply exceeded the saved chat byte limit; the full task transcript remains available");goto done;}if(size<0)goto done;
 if(!native_guard(n,false)||!native_scope(n,&n->connection))goto done;
 r=turn_row(model_db(&n->m),n->m.account,n->m.turn,false);int64_t change=model_matches(&n->m,r)?ctn_delta(ts(PQgetvalue(r,0,6)),tn(text,size),tb(delta,131072)):-1;PQclear(r);if(change<0)goto done;
 if(change>0&&!n->stop&&!model_emit(&n->m,TEXT,tn(delta,change),ts(""),ts(""),false)){goto done;}ok=native_capture_reset(n,NULL);goto done;
 reset:(void)native_capture_reset(n,NULL);
 done:free(c);free(data);free(pool);free(decode);free(text);free(delta);return ok;
}
static void native_cycle(native_turn *n){
 unsigned char body[40961],out[47105],arena[2048];coax_t_ExBinding b=n->task.binding;int64_t size,got;coax_t_XtStatus status={0};bool prepared_here=false;
 if(!native_guard(n,false)){return;}bool scope=native_scope(n,&n->connection);bool observed=false;
 if(n->phase>0&&scope){size=ta_read(b.task,tb(body,sizeof body));got=size>0?native_rpc(n,tn(body,size),out,sizeof out,false):-1;if(got>0)observed=native_observe(n,out,got,ts(""),&status);if(got== -2004)scope=false;}
 int action=(int)ctn_action(n->phase,n->stop,scope,observed,status.state,(int64_t)api_now_ms(),b.expires);
 if(action<0){native_finish(n,action== -4?3:action== -2?2:4,action== -4?"":"Native enrollment or preparation window is unavailable");return;}
 if(action==1){
  size=ex_binding_write(b,tb(body,sizeof body));got=size>0?native_rpc(n,tn(body,size),out,sizeof out,true):-1;coax_t_ExPrepView p={0};
  if(got<1||!ctn_prep_read(tn(out,got),(coax_v_t_ExPrepView){&p,1},tb(arena,sizeof arena))||!ctn_prepared(b,p,b.command,0)){if(got== -2004||got== -2005||got== -2006)native_finish(n,2,"Native preparation was refused");return;}
  int64_t offset=p.retained;while(offset<b.input_bytes){if(!native_guard(n,true))return;if(n->stop){native_finish(n,3,"");return;}size=ctn_chunk(b,ts(n->raw),offset,tb(body,sizeof body));char command[33]={0};if(!ctn_command(b.task,ts("input"),offset,tb(command,32)))return;
   got=size>0?native_rpc(n,tn(body,size),out,sizeof out,true):-1;if(got<1||!ctn_prep_read(tn(out,got),(coax_v_t_ExPrepView){&p,1},tb(arena,sizeof arena))||!ctn_prepared(b,p,ts(command),offset+1)){if(got== -2008)native_finish(n,3,"");else if(got== -2004||got== -2005||got== -2006)native_finish(n,2,"Native input upload was refused");return;}offset=p.retained;
  }
  if(!p.complete||!native_store(n,1,n->prior,n->stop_record)){if(n->stop)native_finish(n,3,"");return;}action=2;prepared_here=true;
 }
 if(action==2){
  coax_t_XtRequest q={.method=ts("StartTask"),.task=b.task,.command=b.command,.issued=b.issued,.expires=b.expires,.route=n->task.dispatch};size=xt_request_write(q,tb(body,sizeof body));got=size>0?native_rpc(n,tn(body,size),out,sizeof out,true):-1;
  if(got>0)observed=native_observe(n,out,got,b.command,&status);else if(got== -2008&&prepared_here)native_finish(n,3,"");else if(got== -2004||got== -2005||got== -2006)native_finish(n,4,"Native Start was refused; reconcile the retained task");if(!observed)return;
 }
 if(observed&&n->stop&&ctn_outcome(status.state)==0){
  int action=(int)ctn_stop_action(b,ts(n->stop_record),ts(n->prior),(int64_t)api_now_ms(),0);
  if(action<0)return;
  if(action==1){size=ctn_stop_request(b,(int64_t)api_now_ms(),tb(body,sizeof body-1));if(size<1)return;body[size]=0;if(!native_store(n,n->phase,n->prior,(char *)body))return;
  }
  if(action!=0){coax_t_XtRequest q;unsigned char qa[2048];if(!xt_request_read(ts(n->stop_record),(coax_v_t_XtRequest){&q,1},tb(qa,sizeof qa)))return;got=native_rpc(n,ts(n->stop_record),out,sizeof out,false);
   if(got>0)observed=native_observe(n,out,got,q.command,&status);
   else if(got== -2005){
    // Retry expired bytes first: a retained native receipt would reconcile even
    // after expiry. Only a refusal plus a new Read may license replacement.
    size=ta_read(b.task,tb(body,sizeof body));int64_t fresh=size>0?native_rpc(n,tn(body,size),out,sizeof out,false):-1;
    observed=fresh>0&&native_observe(n,out,fresh,ts(""),&status);
    if(observed&&ctn_stop_action(b,ts(n->stop_record),ts(n->prior),(int64_t)api_now_ms(),-2005)==1){
     size=ctn_stop_request(b,(int64_t)api_now_ms(),tb(body,sizeof body-1));if(size<1)return;body[size]=0;if(!native_store(n,n->phase,n->prior,(char *)body))return;
    }
   }
  }
 }
 if(observed){int outcome=(int)ctn_outcome(status.state);bool captured=native_capture(n);if(outcome!=0){
   if(outcome==1){if(!captured)return;native_finish(n,1,"");}else native_finish(n,outcome,outcome==2?"Native task failed":outcome==4?"Native task outcome is unknown":"");
  }}
}
static void native_reply(native_turn *n){
 PGconn *db=model_db(&n->m);if(db){char now[32],due[32];long long ms=(long long)api_now_ms();num(now,ms);num(due,ms+5000);const char *v[]={n->m.account,n->m.turn,n->m.dispatch,now,due};
  if(exec_sql(db,"BEGIN",0,NULL)){PGresult *r=turn_row(db,n->m.account,n->m.turn,true);bool ok=model_matches(&n->m,r);const char *poll[]={v[0],v[1],v[4]};if(ok)ok=exec_sql(db,"UPDATE chat.turn_tasks_v1 SET poll_after_ms=$3::bigint WHERE account=$1 AND turn=$2",3,poll)&&exec_sql(db,"UPDATE chat.turns_v2 SET claim_until_ms=0,heartbeat_ms=$4::bigint WHERE account=$1 AND id=$2 AND dispatch=$3",4,v);PQclear(r);exec_sql(db,ok?"COMMIT":"ROLLBACK",0,NULL);}
  PGresult *r=turn_row(db,n->m.account,n->m.turn,false);coax_t_CtView *viewed=calloc(1,sizeof *viewed);unsigned char out[4096];int64_t size=viewed?control(r,viewed,out,sizeof out):-1;if(size>0)reply(&n->m.response,200,out,(size_t)size);else refuse(&n->m.response,503,"Reopen the saved turn to reconcile its native task.");free(viewed);PQclear(r);
 }else refuse(&n->m.response,503,"Native task storage unavailable.");
}
static void native_task_run(const request *q,const char *account,const char *turn,const char *dispatch,const char *worker,const char *key,const char *hosted){
 native_turn n={.m={.response=*q},.deadline=api_now_ms()+300000};snprintf(n.m.account,sizeof n.m.account,"%s",account);snprintf(n.m.turn,sizeof n.m.turn,"%s",turn);snprintf(n.m.dispatch,sizeof n.m.dispatch,"%s",dispatch);snprintf(n.m.worker,sizeof n.m.worker,"%s",worker);snprintf(n.m.key,sizeof n.m.key,"%s",key);
 snprintf(n.file,sizeof n.file,"/tmp/oa-chat-native-XXXXXX");int fd=mkstemp(n.file);if(fd>=0)close(fd);
 if(fd<0||!native_load(&n)){native_finish(&n,4,"Native task input or association is unavailable");goto done;}
 if(!n.record){PGresult *r=turn_row(model_db(&n.m),account,turn,false);char conversation[37]={0};bool owned=model_matches(&n.m,r)&&copy_string(conversation,sizeof conversation,ts(PQgetvalue(r,0,1)));PQclear(r);int selected=owned?native_preflight(&n,conversation):-1;
  if(selected==0){if(n.m.db){PQfinish(n.m.db);n.m.db=NULL;}unlink(n.file);free(n.raw);hosted_model_run(q,account,turn,dispatch,worker,key,hosted);return;}if(selected<0){if(n.stop)native_finish(&n,3,"");else native_finish(&n,2,"Native routing or SDK model discovery is unavailable");goto done;}
 }
 // Resolve the current connection by the full retained host and enrollment.
 copy_string(n.connection.host,sizeof n.connection.host,n.task.binding.host);copy_string(n.connection.digest,sizeof n.connection.digest,n.task.enrollment);
 const char *v[]={account,n.connection.host};PGresult *r=PQexecParams(model_db(&n.m),"SELECT host_pubkey,grant_id,epoch,relay,display_name FROM chat.environment_connections_v1 WHERE account=$1 AND host_pubkey=$2",2,NULL,v,NULL,NULL,0);native_connection current={0};if(tuples(r)&&PQntuples(r)==1&&native_connection_read(&current,r,0)&&!strcmp(current.digest,n.connection.digest))n.connection=current;PQclear(r);
 char hash[65]={0};bool input=pr_hash(ts(n.raw),tb(hash,64)).count==64&&n.task.binding.input_bytes==(int64_t)strlen(n.raw)&&n.task.binding.input_hash.count==64&&!memcmp(n.task.binding.input_hash.data,hash,64);if(!input){native_finish(&n,4,"Admitted native input no longer matches its binding");goto done;}
 native_cycle(&n);
 done:native_reply(&n);if(fd>=0)unlink(n.file);if(n.m.db)PQfinish(n.m.db);free(n.raw);free(n.record);free(n.prior);free(n.stop_record);
}
static void model_run(const request *q,const char *account,const char *turn,const char *dispatch,const char *worker,const char *key,const char *input){
 PGconn *db=api_db_new();const char *v[]={account,turn};PGresult *r=db?PQexecParams(db,"SELECT EXISTS(SELECT 1 FROM chat.turn_tasks_v1 WHERE account=$1 AND turn=$2) OR EXISTS(SELECT 1 FROM chat.environment_connections_v1 WHERE account=$1) FROM chat.turns_v2 WHERE account=$1 AND id=$2",2,NULL,v,NULL,NULL,0):NULL;
 bool native=tuples(r)&&PQntuples(r)==1&&PQgetvalue(r,0,0)[0]=='t';bool available=tuples(r)&&PQntuples(r)==1;PQclear(r);if(db)PQfinish(db);if(!available){refuse(q,503,"Dispatch storage unavailable.");return;}if(!native){hosted_model_run(q,account,turn,dispatch,worker,key,input);return;}
 signal(SIGCHLD,SIG_IGN);pid_t pid=fork();if(pid==0){native_task_run(q,account,turn,dispatch,worker,key,input);_exit(0);}if(pid<0)refuse(q,503,"Native worker unavailable; reconcile the saved turn.");
}
