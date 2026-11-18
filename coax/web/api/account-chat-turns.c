// Platform effects for durable owned chat: parameterized storage, row locks,
// entropy and the existing runner/inference transports. All decisions and JSON
// composition are the shared Coax codec/policy/reducer, never SQL functions.
#include "api.h"
#include "account-chat-wire.generated.h"
#include "projects-wire.generated.h"
#include "environment-process.h"
#include "auth-policy.generated.h"
#include <stdlib.h>
#include <string.h>
#define CT_CAP (1u<<20)
enum {Q=1,RUNNING=2,DONE=3,FAILED=4,CANCELLED=5,UNKNOWN=6};
enum {START=1,TEXT=2,USAGE=3,STOP=4,END=5,ERROR=6,LOST=7,CANCEL=8};
static coax_string ts(const char *s){return (coax_string){s?s:"",s?(int64_t)strlen(s):0};}
static coax_v_u8 tb(void *p,size_t n){return (coax_v_u8){p,(int64_t)n};}
static coax_string tn(const void *p,int64_t n){return (coax_string){p,n};}
static bool tuples(PGresult *r){return r && PQresultStatus(r)==PGRES_TUPLES_OK;}
static bool exec_sql(PGconn *db,const char *sql,int n,const char *const *v){PGresult *r=PQexecParams(db,sql,n,NULL,v,NULL,NULL,0);bool ok=r&&(PQresultStatus(r)==PGRES_COMMAND_OK||tuples(r));PQclear(r);return ok;}
static void num(char out[32],long long n){snprintf(out,32,"%lld",n);}
static bool copy_string(char *out,size_t cap,coax_string s){if(s.count<0||(size_t)s.count>=cap)return false;memcpy(out,s.data,(size_t)s.count);out[s.count]=0;return true;}
static void reply(const request *q,int status,const void *body,size_t n){api_respond(q->fd,status,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",body,n);}
static void refuse(const request *q,int status,const char *message){api_error(q->fd,status,message);}
static long long number(PGresult *r,int row,int col){return atoll(PQgetvalue(r,row,col));}
static long long nullable(PGresult *r,int row,int col){return PQgetisnull(r,row,col)?-1:number(r,row,col);}
#define TURN_FIELDS "t.account,t.conversation,t.id,t.state,t.stop_requested,t.sequence,t.output,t.stop_reason,t.error,t.input_tokens,t.output_tokens,t.cache_read_tokens,t.cache_write_tokens,t.reasoning_tokens,t.operation,t.model,t.provider,t.effort,coalesce(t.snapshot_id,''),t.created_ms,t.updated_ms,c.revision,coalesce(t.worker,''),coalesce(t.runner_key,''),coalesce(t.claim_operation,''),coalesce(t.dispatch,''),t.claim_until_ms,t.dispatched_ms,t.heartbeat_ms,t.input_json,coalesce(c.project,'')"
// One turn's row: TURN_FIELDS (0 to 30), then 31, what it was admitted for (migration 0214).
static PGresult *turn_row(PGconn *db,const char *account,const char *id,bool lock){const char *v[]={account,id};return PQexecParams(db,lock?"SELECT " TURN_FIELDS ",t.admitted_for FROM chat.turns_v2 t JOIN chat.conversations_v2 c ON c.account=t.account AND c.id=t.conversation WHERE t.account=$1 AND t.id=$2 FOR UPDATE OF t":"SELECT " TURN_FIELDS ",t.admitted_for FROM chat.turns_v2 t JOIN chat.conversations_v2 c ON c.account=t.account AND c.id=t.conversation WHERE t.account=$1 AND t.id=$2",2,NULL,v,NULL,NULL,0);}
static bool view(PGresult *r,int row,coax_t_CtView *v){if(!tuples(r)||row>=PQntuples(r))return false;coax_t_CtUsage u={.input=nullable(r,row,9),.output=nullable(r,row,10),.cache_read=nullable(r,row,11),.cache_write=nullable(r,row,12),.reasoning=nullable(r,row,13)};return cta_restore((coax_v_t_CtView){v,1},ts(PQgetvalue(r,row,1)),ts(PQgetvalue(r,row,2)),number(r,row,3),number(r,row,5),PQgetvalue(r,row,4)[0]=='t',ts(PQgetvalue(r,row,6)),ts(PQgetvalue(r,row,7)),ts(PQgetvalue(r,row,8)),(coax_v_t_CtUsage){&u,1});}
static int64_t control(PGresult *r,coax_t_CtView *v,unsigned char *out,size_t cap){return view(r,0,v)?ctr_control_write((coax_v_t_CtView){v,1},number(r,0,21),tb(out,cap)):-1;}
static bool save_view(PGconn *db,PGresult *r,coax_t_CtView *v,coax_t_CtEvent *event){
 unsigned char *wire=calloc(1,131073);char *text=strndup((char *)v->content.e,(size_t)v->bytes),*reason=strndup((char *)v->reason.e,(size_t)v->nr),*error=strndup((char *)v->error.e,(size_t)v->ne);if(!wire||!text||!reason||!error){free(wire);free(text);free(reason);free(error);return false;}
 int64_t n=ct_event_write((coax_v_t_CtEvent){event,1},tb(wire,131072));char state[32],seq[32],before[32],ms[32],u[5][32];num(state,v->state);num(seq,v->sequence);num(before,number(r,0,5));num(ms,(long long)api_now_ms());long long counts[]={v->usage.input,v->usage.output,v->usage.cache_read,v->usage.cache_write,v->usage.reasoning};for(int i=0;i<5;i++){u[i][0]=0;if(counts[i]>=0)num(u[i],counts[i]);}
 const char *p[]={PQgetvalue(r,0,0),PQgetvalue(r,0,2),state,v->stop_requested?"true":"false",seq,text,reason,error,u[0],u[1],u[2],u[3],u[4],ms,before};
 PGresult *w=n>0?PQexecParams(db,"UPDATE chat.turns_v2 SET state=$3::int,stop_requested=$4::bool,sequence=$5::bigint,output=$6,stop_reason=$7,error=$8,input_tokens=nullif($9,'')::bigint,output_tokens=nullif($10,'')::bigint,cache_read_tokens=nullif($11,'')::bigint,cache_write_tokens=nullif($12,'')::bigint,reasoning_tokens=nullif($13,'')::bigint,updated_ms=$14::bigint,heartbeat_ms=$14::bigint WHERE account=$1 AND id=$2 AND sequence=$15::bigint RETURNING id",15,NULL,p,NULL,NULL,0):NULL;
 bool ok=tuples(w)&&PQntuples(w)==1;PQclear(w);const char *ev[]={p[0],p[1],seq,(char *)wire};if(ok)ok=exec_sql(db,"INSERT INTO chat.turn_events_v2(account,turn,sequence,record) VALUES($1,$2,$3::bigint,$4)",4,ev);
 const char *cv[]={p[0],PQgetvalue(r,0,1),ms};if(ok)ok=exec_sql(db,"UPDATE chat.conversations_v2 SET updated_ms=$3::bigint WHERE account=$1 AND id=$2",3,cv);
 free(wire);free(text);free(reason);free(error);return ok;
}
static bool operation_lock(PGconn *db,const char *account,const char *operation){const char *p[]={account,operation};return exec_sql(db,"SELECT pg_advisory_xact_lock(hashtextextended('durable:'||$1||':'||$2,0))",2,p);}
static int operation(PGconn *db,const char *account,const char *op,const char *path,const char *hash,char id[37]){const char *p[]={account,op};PGresult *r=PQexecParams(db,"SELECT path,digest,turn FROM chat.turn_operations_v2 WHERE account=$1 AND operation=$2",2,NULL,p,NULL,NULL,0);if(!tuples(r)){PQclear(r);return -1;}bool found=PQntuples(r)>0;int result=(int)ct_operation(found,found&&!strcmp(PQgetvalue(r,0,0),path)&&!strcmp(PQgetvalue(r,0,1),hash));if(found&&result==2)snprintf(id,37,"%s",PQgetvalue(r,0,2));PQclear(r);return result;}
static bool operation_save(PGconn *db,const char *account,const char *op,const char *path,const char *hash,const char *id){const char *p[]={account,op,path,hash,id};return exec_sql(db,"INSERT INTO chat.turn_operations_v2(account,operation,path,digest,turn) VALUES($1,$2,$3,$4,$5)",5,p);}
// admitted_for: "" for a durable chat turn, "gamebuilder-agent" from a GameBuilder build agent's start
// (gamebuilder-agents.c). It is stored on the turn and is what funds it again at dispatch (act_funded_turn).
static void submit(const request *q,PGconn *db,const auth_principal *p,const char *admitted_for){
 coax_t_CtSubmit *s=calloc(1,sizeof *s);coax_t_CtInput *h=calloc(1,sizeof *h);coax_t_CtView *v=calloc(1,sizeof *v);unsigned char *out=calloc(1,CT_CAP+1),*native=calloc(1,CT_CAP+1),arena[174];project_context context={0};PGresult *c=NULL,*r=NULL;int status=503;const char *why="Chat storage unavailable.";bool tx=false;char op[81]="",cid[37]="",tid[37]="",project[37]="",legacy[23]="",hash[65]="",title[161]="",ms[32],revision[32],legacy_revision[32];char *legacy_record=NULL,*canonical_request=NULL;
 if(!s||!h||!v||!out||!native){goto done;}
 if(!ct_submit_read(tn(q->body,q->body_len),(coax_v_t_CtSubmit){s,1},tb(arena,sizeof arena))){status=400;why="Invalid durable chat request.";goto done;}
 copy_string(op,sizeof op,s->operation);copy_string(cid,sizeof cid,s->conversation);copy_string(project,sizeof project,s->project);copy_string(legacy,sizeof legacy,s->legacy);
 int64_t n=cti_canonical(tn(q->body,q->body_len),tb(out,131072));if(n<1){status=400;goto done;}(void)pr_hash(tn(out,n),tb(hash,64));canonical_request=strndup((char *)out,(size_t)n);if(!canonical_request){goto done;}
 tx=exec_sql(db,"BEGIN",0,NULL);if(!tx||!operation_lock(db,p->account,op)){goto done;}
 int prior=operation(db,p->account,op,q->path,hash,tid);if(prior<0){goto done;}if(prior==0){status=409;why="Operation ID already has different contents.";goto done;}
 if(prior==2){r=turn_row(db,p->account,tid,false);n=control(r,v,out,CT_CAP);if(n<1){goto done;}status=202;reply(q,status,out,(size_t)n);goto replied;}
 // A limited session submits only where ap_limited_write names this route (a GameBuilder build agent; never /api/chat/submit/v2alpha1).
 bool limited=p->limited&&!ap_limited_write(ts(q->method),ts(q->path));
 if(!act_turn_for(ts(admitted_for))||!ct_admit(true,act_funded_turn(p->kind,ts(p->account),p->team,ts(admitted_for)),limited,p->frozen!=0,false,true,true)){status=403;why="Agent chat is currently available to the unrestricted team beta.";goto done;}
 const char *owner[]={p->account};if(!exec_sql(db,"SELECT pg_advisory_xact_lock(hashtextextended('durable-account:'||$1,0))",1,owner)){goto done;}r=PQexecParams(db,"SELECT count(*) FROM chat.turns_v2 WHERE account=$1 AND state IN (1,2)",1,NULL,owner,NULL,NULL,0);bool capacity=tuples(r)&&PQntuples(r)==1&&ctw_queue(number(r,0,0));PQclear(r);r=NULL;if(!capacity){status=429;why="32 chats are queued or running on this account, the most at once. Stop or finish one before queuing more.";goto done;}
 bool fresh=!*cid;if(fresh&&!api_object_id(cid,"cnv_")){goto done;}if(!api_object_id(tid,"trn_")){goto done;}long long now=(long long)api_now_ms();num(ms,now);
 if(*project){status=projects_context(db,p->account,project,cid,op,s->project_revision,now,&context);if(status!=200){why=status==404?"not found":status==409?"Project changed. Reopen it before sending.":"The complete Project context does not fit.";goto done;}status=503;}
 long long before=0;
 if(!fresh){const char *cv[]={p->account,cid};c=PQexecParams(db,"SELECT revision,coalesce(project,''),coalesce(legacy_record,'') FROM chat.conversations_v2 WHERE account=$1 AND id=$2 FOR UPDATE",2,NULL,cv,NULL,NULL,0);if(!tuples(c)){goto done;}if(PQntuples(c)!=1){status=404;why="not found";goto done;}before=number(c,0,0);if(strcmp(project,PQgetvalue(c,0,1))){status=409;why="Keep this conversation's saved Project association.";goto done;}legacy_record=strdup(PQgetvalue(c,0,2));}
 else if(*legacy){const char *lv[]={p->account,legacy};r=PQexecParams(db,"SELECT t.record,t.revision,coalesce(l.project,j.project,'') FROM chat.account_threads_v1 t LEFT JOIN projects.thread_links_v1 l ON l.account=t.account AND l.thread=t.id LEFT JOIN chat.project_threads_v1 j ON j.account=t.account AND j.thread=t.id WHERE t.account=$1 AND t.id=$2 FOR UPDATE OF t",2,NULL,lv,NULL,NULL,0);if(!tuples(r)){goto done;}if(PQntuples(r)!=1){status=404;why="not found";goto done;}if(number(r,0,1)!=s->legacy_revision||(*PQgetvalue(r,0,2)&&strcmp(project,PQgetvalue(r,0,2)))){status=409;why="Legacy conversation or its Project association changed.";goto done;}legacy_record=strdup(PQgetvalue(r,0,0));PQclear(r);r=NULL;}
 if(!ctw_revision(s->revision,before)){status=409;why="Conversation changed. Reopen it before sending.";goto done;}
 const char *hv[]={p->account,cid};r=PQexecParams(db,"SELECT request_json,output,state FROM chat.turns_v2 WHERE account=$1 AND conversation=$2 ORDER BY ordinal LIMIT 129",2,NULL,hv,NULL,NULL,0);if(!tuples(r)){goto done;}bool active=false;bool input=cti_legacy((coax_v_t_CtInput){h,1},ts(legacy_record));
 for(int i=0;i<PQntuples(r);i++){if(!ct_terminal(number(r,i,2)))active=true;if(input)input=cti_history((coax_v_t_CtInput){h,1},ts(PQgetvalue(r,i,0)),ts(PQgetvalue(r,i,1)),number(r,i,2));}PQclear(r);r=NULL;
 if(active){status=409;why="This conversation already has an active turn.";goto done;}
 input=input&&cti_submit((coax_v_t_CtInput){h,1},tn(q->body,q->body_len));n=input?cti_write((coax_v_t_CtInput){h,1},ts(context.instructions),ts(context.reference),tb(out,CT_CAP)):-1;
 if(!ct_admit(true,true,limited,p->frozen!=0,active,n>0,true)){status=413;why="The complete conversation and Project context do not fit. Start a new conversation.";goto done;}out[n]=0;
 int64_t native_n=ctn_input((coax_v_bool){h->roles.e,128},(coax_v_s64){h->starts.e,128},(coax_v_s64){h->counts.e,128},tn(h->data.e,h->bytes),h->n,ts(context.instructions),ts(context.reference),tb(native,CT_CAP));
 if(native_n<1){status=413;why="The complete native conversation and Project context do not fit.";goto done;}native[native_n]=0;
 int64_t title_n=cti_title(tn(q->body,q->body_len),tb(title,160));if(title_n<0){goto done;}num(revision,before+1);num(legacy_revision,s->legacy_revision);
 if(fresh){const char *cv[]={cid,p->account,project,title,legacy,legacy_revision,legacy_record,ms};if(!exec_sql(db,"INSERT INTO chat.conversations_v2(id,account,project,revision,title,legacy_thread,legacy_revision,legacy_record,created_ms,updated_ms) VALUES($1,$2,nullif($3,''),1,$4,nullif($5,''),CASE WHEN $5<>'' THEN $6::bigint END,$7,$8::bigint,$8::bigint)",8,cv)){goto done;}}
 else {const char *cv[]={p->account,cid,revision,ms};if(!exec_sql(db,"UPDATE chat.conversations_v2 SET revision=$3::bigint,updated_ms=$4::bigint WHERE account=$1 AND id=$2",4,cv)){goto done;}}
 const char *tv[]={tid,p->account,cid,revision,op,hash,canonical_request,(char *)out,context.snapshot_id,context.snapshot,ms,(char *)native,admitted_for};
 if(!exec_sql(db,"INSERT INTO chat.turns_v2(id,account,conversation,ordinal,operation,digest,request_json,input_json,snapshot_id,snapshot_json,state,model,provider,effort,created_ms,updated_ms,native_input_json,admitted_for) VALUES($1,$2,$3,$4::bigint,$5,$6,$7,$8,nullif($9,''),$10,1,'google/gemini-4-hydrogen','google','low',$11::bigint,$11::bigint,$12,$13)",13,tv)||!operation_save(db,p->account,op,q->path,hash,tid)||!exec_sql(db,"COMMIT",0,NULL)){goto done;}tx=false;
 r=turn_row(db,p->account,tid,false);n=control(r,v,out,CT_CAP);if(n<1){goto done;}(void)chat_wake();reply(q,202,out,(size_t)n);goto replied;
 done: if(tx)exec_sql(db,"ROLLBACK",0,NULL);refuse(q,status,why);
 replied: if(tx)exec_sql(db,"ROLLBACK",0,NULL);PQclear(c);PQclear(r);free(legacy_record);free(canonical_request);projects_context_free(&context);free(s);free(h);free(v);free(out);free(native);
}
static void stop_turn(const request *q,PGconn *db,const auth_principal *p){
 unsigned char opb[81]={0},id[37]={0};int64_t no=ct_stop_read(tn(q->body,q->body_len),tb(opb,80),tb(id,36));if(no<1){refuse(q,400,"Invalid Stop request.");return;}char hash[65]={0};unsigned char canonical[1024];int64_t canonical_n=ct_stop_write(tn(opb,no),tn(id,36),tb(canonical,sizeof canonical));if(canonical_n<1){refuse(q,400,"Invalid Stop request.");return;}(void)pr_hash(tn(canonical,canonical_n),tb(hash,64));
 coax_t_CtView *v=calloc(1,sizeof *v);unsigned char *out=calloc(1,CT_CAP);PGresult *r=NULL;int status=503;const char *why="Stop could not be saved.";bool tx=exec_sql(db,"BEGIN",0,NULL);char prior_id[37];if(!v||!out||!tx||!operation_lock(db,p->account,(char *)opb)){goto done;}
 r=turn_row(db,p->account,(char *)id,true);if(!tuples(r)){goto done;}if(PQntuples(r)!=1){status=404;why="not found";goto done;}if(!view(r,0,v)){goto done;}
 int prior=operation(db,p->account,(char *)opb,q->path,hash,prior_id);if(prior<0){goto done;}if(prior==0){status=409;why="Operation ID already has different contents.";goto done;}
 if(prior==1){coax_t_CtEvent event;coax_t_CtUsage usage={0};int result=(int)cta_event((coax_v_t_CtView){v,1},STOP,ts(""),ts(""),ts(""),false,(coax_v_t_CtUsage){&usage,1},(coax_v_t_CtEvent){&event,1});if(result!=2&&(result!=1||!save_view(db,r,v,&event))){goto done;}if(!operation_save(db,p->account,(char *)opb,q->path,hash,(char *)id)){goto done;}}
 const char *wake[]={p->account,(char *)id};if(!exec_sql(db,"UPDATE chat.turn_tasks_v1 SET poll_after_ms=0 WHERE account=$1 AND turn=$2",2,wake)||!exec_sql(db,"COMMIT",0,NULL)){goto done;}tx=false;PQclear(r);r=turn_row(db,p->account,(char *)id,false);int64_t n=control(r,v,out,CT_CAP);if(n<1){goto done;}(void)chat_wake();reply(q,200,out,(size_t)n);goto finish;
 done: if(tx)exec_sql(db,"ROLLBACK",0,NULL);refuse(q,status,why);
 finish: PQclear(r);free(out);free(v);
}
static void read_turn(const request *q,PGconn *db,const auth_principal *p,bool events){
 char id[64],after_text[32];api_query(q,"id",id,sizeof id);api_query(q,"after",after_text,sizeof after_text);long long after=cta_cursor(ts(after_text));if(!cta_id(ts(id),true)||after<0){refuse(q,400,"Invalid turn or event cursor.");return;}
 // One MVCC cut for materialization/cursor and its event page.
 if(!exec_sql(db,"BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",0,NULL)){refuse(q,503,"Chat storage unavailable.");return;}PGresult *r=turn_row(db,p->account,id,false),*er=NULL;coax_t_CtView *v=calloc(1,sizeof *v);unsigned char *out=calloc(1,CT_CAP);int status=503;const char *why="Chat storage unavailable.";int64_t n=-1;
 if(!v||!out||!tuples(r)){goto done;}if(PQntuples(r)!=1){status=404;why="not found";goto done;}if(!view(r,0,v)){goto done;}
 if(!events){coax_t_CtInfo info={.operation=ts(PQgetvalue(r,0,14)),.model=ts(PQgetvalue(r,0,15)),.provider=ts(PQgetvalue(r,0,16)),.effort=ts(PQgetvalue(r,0,17)),.route=ts(""),.why=ts("Account chat beta"),.snapshot=ts(PQgetvalue(r,0,18)),.revision=number(r,0,21),.created=number(r,0,19),.updated=number(r,0,20)};n=ctr_turn_write((coax_v_t_CtView){v,1},(coax_v_t_CtInfo){&info,1},tb(out,CT_CAP));}
 else {if(after>v->sequence){status=409;why="The event cursor is ahead of this turn.";goto done;}char cursor[32];num(cursor,after);const char *ev[]={p->account,id,cursor};er=PQexecParams(db,"SELECT record FROM chat.turn_events_v2 WHERE account=$1 AND turn=$2 AND sequence>$3::bigint ORDER BY sequence LIMIT 32",3,NULL,ev,NULL,NULL,0);if(!tuples(er)){goto done;}coax_string records[32];int count=0;size_t bytes=0;for(int i=0;i<PQntuples(er);i++){size_t size=strlen(PQgetvalue(er,i,0));if(count&&bytes+size>100000)break;records[count++]=ts(PQgetvalue(er,i,0));bytes+=size;}n=ctr_events((coax_v_t_CtView){v,1},after,(coax_v_string){records,count},tb(out,131072));}
 if(n<1){goto done;}reply(q,200,out,(size_t)n);goto finish;
 done: refuse(q,status,why);
 finish: exec_sql(db,"ROLLBACK",0,NULL);PQclear(er);PQclear(r);free(out);free(v);
}
static void read_admitted(const request *q,PGconn *db,const auth_principal *p,int kind){
 char id[64];api_query(q,"id",id,sizeof id);if(!cta_id(ts(id),true)){refuse(q,400,"Invalid turn.");return;}
 const char *sql=kind==8?"SELECT snapshot_json FROM chat.turns_v2 WHERE account=$1 AND id=$2":kind==9?"SELECT input_json FROM chat.turns_v2 WHERE account=$1 AND id=$2":kind==10?"SELECT native_input_json FROM chat.turns_v2 WHERE account=$1 AND id=$2":"SELECT x.record,t.conversation FROM chat.turn_tasks_v1 x JOIN chat.turns_v2 t ON t.account=x.account AND t.id=x.turn WHERE x.account=$1 AND x.turn=$2";
 const char *args[]={p->account,id};PGresult *r=PQexecParams(db,sql,2,NULL,args,NULL,NULL,0);
 if(!tuples(r)){refuse(q,503,"Chat storage unavailable.");}
 else if(PQntuples(r)!=1||PQgetisnull(r,0,0)||!PQgetlength(r,0,0)){refuse(q,404,"not found");}
 else if((size_t)PQgetlength(r,0,0)>CT_CAP){refuse(q,503,"Saved input exceeds its transport bound.");}
 // These bytes were composed by the sole Project/gateway codec at admission.
 // Inspection performs no reconstruction from the Project's current state.
 else if(kind==11){coax_t_CttTask task;unsigned char arena[20480];bool valid=ctt_read(ts(PQgetvalue(r,0,0)),(coax_v_t_CttTask){&task,1},tb(arena,sizeof arena))&&ctt_owned(task,ts(id),ts(PQgetvalue(r,0,1)));if(valid)reply(q,200,PQgetvalue(r,0,0),(size_t)PQgetlength(r,0,0));else refuse(q,503,"Saved task association is invalid.");}
 else reply(q,200,PQgetvalue(r,0,0),(size_t)PQgetlength(r,0,0));
 PQclear(r);
}
static void conversation_index(const request *q,PGconn *db,const auth_principal *p){
 char cursor[81],project[37];api_query(q,"cursor",cursor,sizeof cursor);api_query(q,"project_id",project,sizeof project);if((*cursor&&!cta_id(ts(cursor),false))||(*project&&!prp_id(ts(project)))){refuse(q,400,"Invalid conversation cursor or Project.");return;}
 const char *args[]={p->account,cursor,project};PGresult *r=PQexecParams(db,"SELECT c.id,c.title,c.revision,coalesce(c.project,''),coalesce((SELECT id FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id AND t.state IN(1,2)),''),c.created_ms,c.updated_ms FROM chat.conversations_v2 c WHERE c.account=$1 AND ($2='' OR c.id>$2) AND ($3='' OR c.project=$3) ORDER BY c.id LIMIT 61",3,NULL,args,NULL,NULL,0);
 unsigned char *out=calloc(1,131072);char *owned[60]={0};coax_string items[60];int count=0;bool ok=out&&tuples(r);for(int i=0;ok&&i<PQntuples(r)&&i<60;i++){coax_t_CtConversation c={.id=ts(PQgetvalue(r,i,0)),.title=ts(PQgetvalue(r,i,1)),.revision=number(r,i,2),.project=ts(PQgetvalue(r,i,3)),.active=ts(PQgetvalue(r,i,4)),.legacy_revision=-1,.created=number(r,i,5),.updated=number(r,i,6)};int64_t n=ctr_summary((coax_v_t_CtConversation){&c,1},tb(out,131071));if(n<1){ok=false;break;}out[n]=0;owned[count]=strdup((char *)out);ok=owned[count]!=NULL;if(ok)items[count]=ts(owned[count]);count++;}
 int64_t n=ok?ctr_index((coax_v_string){items,count},ts(PQntuples(r)>60?PQgetvalue(r,59,0):""),tb(out,131072)):-1;if(n>0)reply(q,200,out,(size_t)n);else refuse(q,503,"Conversation history unavailable.");for(int i=0;i<count;i++)free(owned[i]);free(out);PQclear(r);
}
static void conversation_read(const request *q,PGconn *db,const auth_principal *p){
 char id[64],cursor[32];api_query(q,"id",id,sizeof id);api_query(q,"after",cursor,sizeof cursor);long long after=cta_cursor(ts(cursor));if(!cta_id(ts(id),false)||after<0){refuse(q,400,"Invalid conversation cursor.");return;}
 if(!exec_sql(db,"BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",0,NULL)){refuse(q,503,"History unavailable.");return;}
 const char *args[]={p->account,id};PGresult *r=PQexecParams(db,"SELECT c.id,c.revision,coalesce(c.project,''),coalesce(c.legacy_thread,''),c.legacy_revision,coalesce(c.legacy_record,''),c.created_ms,c.updated_ms,coalesce((SELECT id FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id AND t.state IN(1,2)),'') FROM chat.conversations_v2 c WHERE c.account=$1 AND c.id=$2",2,NULL,args,NULL,NULL,0),*turns=NULL;
 unsigned char *out=calloc(1,CT_CAP);coax_t_CtView *v=calloc(1,sizeof *v);char *owned[32]={0};coax_string messages[32];int count=0,status=503;const char *why="Conversation unavailable.";long long position=after,total=0;size_t bytes=0;int64_t n=-1;
 if(!out||!v||!tuples(r)){goto done;}if(PQntuples(r)!=1){status=404;why="not found";goto done;}int64_t old=cti_legacy_count(ts(PQgetvalue(r,0,5)));if(old<0){goto done;}
 const char *tv[]={p->account,id};PGresult *size=PQexecParams(db,"SELECT count(*) FROM chat.turns_v2 WHERE account=$1 AND conversation=$2",2,NULL,tv,NULL,NULL,0);bool sizes=tuples(size)&&PQntuples(size)==1;if(sizes)total=old+2*number(size,0,0);PQclear(size);if(!sizes){goto done;}if(after>total){status=409;why="History cursor is ahead of this conversation.";goto done;}
 while(position<old&&count<32){n=cti_legacy_message(ts(PQgetvalue(r,0,5)),ts(id),position,number(r,0,6),tb(out,CT_CAP-1));if(n<1){goto done;}if(count&&bytes+(size_t)n>900000)break;out[n]=0;owned[count]=strdup((char *)out);if(!owned[count]){goto done;}messages[count]=ts(owned[count]);count++;bytes+=(size_t)n;position++;}
 if(position>=old&&count<32){char offset[32];num(offset,(position-old)/2);const char *hv[]={p->account,id,offset};turns=PQexecParams(db,"SELECT " TURN_FIELDS ",t.request_json FROM chat.turns_v2 t JOIN chat.conversations_v2 c ON c.account=t.account AND c.id=t.conversation WHERE t.account=$1 AND t.conversation=$2 ORDER BY t.ordinal OFFSET $3::bigint LIMIT 17",3,NULL,hv,NULL,NULL,0);if(!tuples(turns)){goto done;}
  bool full=false;for(int i=0;i<PQntuples(turns)&&!full;i++){if(!view(turns,i,v)){goto done;}int first=(int)((position-old)%2);for(int part=first;part<2;part++){if(count==32){full=true;break;}n=part==0?ctr_user_message(ts(PQgetvalue(turns,i,31)),ts(PQgetvalue(turns,i,2)),number(turns,i,19),tb(out,CT_CAP-1)):ctr_answer_message((coax_v_t_CtView){v,1},number(turns,i,20),tb(out,CT_CAP-1));if(n<1){goto done;}if(count&&bytes+(size_t)n>900000){full=true;break;}out[n]=0;owned[count]=strdup((char *)out);if(!owned[count]){goto done;}messages[count]=ts(owned[count]);count++;bytes+=(size_t)n;position++;}}
 }
 char next[32]="";if(position<total)num(next,position);coax_t_CtConversation c={.id=ts(id),.revision=number(r,0,1),.project=ts(PQgetvalue(r,0,2)),.legacy=ts(PQgetvalue(r,0,3)),.legacy_revision=nullable(r,0,4),.created=number(r,0,6),.updated=number(r,0,7),.active=ts(PQgetvalue(r,0,8)),.cursor=ts(next)};n=ctr_conversation_write((coax_v_t_CtConversation){&c,1},(coax_v_string){messages,count},tb(out,CT_CAP));if(n<1){goto done;}reply(q,200,out,(size_t)n);goto finish;
 done: refuse(q,status,why);
 finish: exec_sql(db,"ROLLBACK",0,NULL);for(int i=0;i<count;i++)free(owned[i]);PQclear(turns);PQclear(r);free(out);free(v);
}
static bool recover(PGconn *db,long long now){
 char cutoff[32];num(cutoff,ctw_cutoff(now));const char *cut[]={cutoff};PGresult *rows=PQexecParams(db,"SELECT account,id FROM chat.turns_v2 t WHERE state=2 AND heartbeat_ms<=$1::bigint AND NOT EXISTS(SELECT 1 FROM chat.turn_tasks_v1 x WHERE x.account=t.account AND x.turn=t.id) ORDER BY heartbeat_ms,id LIMIT 32",1,NULL,cut,NULL,NULL,0);bool ok=tuples(rows);
 for(int i=0;ok&&i<PQntuples(rows);i++){PGresult *r=turn_row(db,PQgetvalue(rows,i,0),PQgetvalue(rows,i,1),true);coax_t_CtView *v=calloc(1,sizeof *v);ok=v&&view(r,0,v);if(ok&&ctw_lost(v->state,!PQgetisnull(r,0,27),nullable(r,0,28),now)){coax_t_CtEvent e;coax_t_CtUsage u={0};ok=cta_event((coax_v_t_CtView){v,1},LOST,ts(""),ts("unknown"),ts("Model worker lost after dispatch"),false,(coax_v_t_CtUsage){&u,1},(coax_v_t_CtEvent){&e,1})==1&&save_view(db,r,v,&e);}free(v);PQclear(r);}PQclear(rows);return ok;
}
static void claim(const request *q,PGconn *db,const char *key,const char *operation,const char *worker){
 bool tx=exec_sql(db,"BEGIN",0,NULL);PGresult *r=NULL,*job=NULL;unsigned char out[4096];int status=503;const char *why="Queue unavailable.";char ms[32],until[32],dispatch[37];long long now=(long long)api_now_ms();num(ms,now);num(until,ctw_claim_end(now));
 if(!tx||!exec_sql(db,"SELECT pg_advisory_xact_lock(hashtextextended('durable-next',0))",0,NULL)||!recover(db,now)){goto done;}
 const char *op[]={key,operation};r=PQexecParams(db,"SELECT x.turn,t.conversation,x.dispatch,x.worker FROM chat.turn_claims_v2 x JOIN chat.turns_v2 t ON t.account=x.account AND t.id=x.turn WHERE x.runner_key=$1 AND x.operation=$2",2,NULL,op,NULL,NULL,0);if(!tuples(r)){goto done;}
 int64_t n=-1;if(PQntuples(r)){if(strcmp(PQgetvalue(r,0,3),worker)){status=409;why="Claim operation belongs to a different worker.";goto done;}n=cwr_job(ts(PQgetvalue(r,0,0)),ts(PQgetvalue(r,0,1)),ts(PQgetvalue(r,0,2)),tb(out,sizeof out));}
 else {
  const char *clock[]={ms};job=PQexecParams(db,"SELECT t.account,t.id,x.turn IS NOT NULL,coalesce(x.poll_after_ms,0) FROM chat.turns_v2 t LEFT JOIN chat.turn_tasks_v1 x ON x.account=t.account AND x.turn=t.id WHERE (t.state=1 OR (t.state=2 AND x.turn IS NOT NULL)) AND (t.claim_until_ms IS NULL OR t.claim_until_ms<=$1::bigint) AND coalesce(x.poll_after_ms,0)<=$1::bigint ORDER BY t.updated_ms,t.id LIMIT 1 FOR UPDATE OF t SKIP LOCKED",1,NULL,clock,NULL,NULL,0);if(!tuples(job)){goto done;}
  if(!PQntuples(job))n=cwr_job(ts(""),ts(""),ts(""),tb(out,sizeof out));
  else {PGresult *t=turn_row(db,PQgetvalue(job,0,0),PQgetvalue(job,0,1),true);bool available=tuples(t)&&PQntuples(t)==1&&ctn_claim(number(t,0,3),PQgetvalue(job,0,2)[0]=='t',number(job,0,3),nullable(t,0,26),now)&&api_object_id(dispatch,"run_");
   const char *v[]={PQgetvalue(job,0,0),PQgetvalue(job,0,1),worker,key,operation,dispatch,until,ms};
   if(available){available=exec_sql(db,"UPDATE chat.turns_v2 SET worker=$3,runner_key=$4,claim_operation=$5,dispatch=$6,claim_until_ms=$7::bigint,updated_ms=$8::bigint WHERE account=$1 AND id=$2",8,v);}
   const char *receipt[]={v[0],v[1],v[2],v[3],v[4],v[5],v[7]};if(available)available=exec_sql(db,"INSERT INTO chat.turn_claims_v2(account,turn,worker,runner_key,operation,dispatch,created_ms) VALUES($1,$2,$3,$4,$5,$6,$7::bigint)",7,receipt);
   if(available){n=cwr_job(ts(PQgetvalue(t,0,2)),ts(PQgetvalue(t,0,1)),ts(dispatch),tb(out,sizeof out));}
   PQclear(t);
  }
 }
 if(n<1||!exec_sql(db,"COMMIT",0,NULL)){goto done;}tx=false;reply(q,200,out,(size_t)n);goto finish;
 done: if(tx)exec_sql(db,"ROLLBACK",0,NULL);refuse(q,status,why);
 finish: PQclear(r);PQclear(job);
}
static void model_run(const request *q,const char *account,const char *turn,const char *dispatch,const char *worker,const char *key,const char *input);
static void dispatch_turn(const request *q,PGconn *db,const char *key,const char *id,const char *dispatch,const char *worker,bool lost){
 bool tx=exec_sql(db,"BEGIN",0,NULL);int status=503;const char *why="Model attempt unavailable.";PGresult *owner=NULL,*r=NULL,*grant=NULL;coax_t_CtView *v=calloc(1,sizeof *v);unsigned char *out=calloc(1,CT_CAP);char *input=NULL;char account[41]="";
 if(!tx||!v||!out){goto done;}owner=PQexecParams(db,"SELECT account FROM chat.turns_v2 WHERE id=$1",1,NULL,&id,NULL,NULL,0);if(!tuples(owner)){goto done;}if(PQntuples(owner)!=1){status=404;why="not found";goto done;}snprintf(account,sizeof account,"%s",PQgetvalue(owner,0,0));r=turn_row(db,account,id,true);if(!view(r,0,v)){goto done;}
 bool matching=!strcmp(PQgetvalue(r,0,22),worker)&&!strcmp(PQgetvalue(r,0,23),key)&&!strcmp(PQgetvalue(r,0,25),dispatch);if(!matching){status=409;why="Worker claim was replaced.";goto done;}
 const char *nv[]={account,id};PGresult *native_row=PQexecParams(db,"SELECT 1 FROM chat.turn_tasks_v1 WHERE account=$1 AND turn=$2",2,NULL,nv,NULL,NULL,0);bool native_storage=tuples(native_row),native=native_storage&&PQntuples(native_row)==1;PQclear(native_row);if(!native_storage)goto done;
 long long now=(long long)api_now_ms();bool started=!PQgetisnull(r,0,27);coax_t_CtEvent e;coax_t_CtUsage u={0};
 if(lost){if(!native&&ctw_lost(v->state,started,nullable(r,0,28),now)){if(cta_event((coax_v_t_CtView){v,1},LOST,ts(""),ts("unknown"),ts("Worker lost after dispatch"),false,(coax_v_t_CtUsage){&u,1},(coax_v_t_CtEvent){&e,1})!=1||!save_view(db,r,v,&e)){goto done;}}}
 else if(!started&&!ct_terminal(v->state)){
  const char *g[]={account,PQgetvalue(r,0,30)};grant=PQexecParams(db,"SELECT kind,team,frozen_at IS NOT NULL,deleted_at IS NOT NULL,($2='' OR EXISTS(SELECT 1 FROM projects.private_v1 WHERE account=$1 AND id=$2)) FROM auth.accounts WHERE id=$1",2,NULL,g,NULL,NULL,0);
  bool allowed=tuples(grant)&&PQntuples(grant)==1&&ctw_grant(act_funded_turn(!strcmp(PQgetvalue(grant,0,0),"person")?2:0,ts(account),PQgetvalue(grant,0,1)[0]=='t',ts(PQgetvalue(r,0,31))),PQgetvalue(grant,0,2)[0]=='t',PQgetvalue(grant,0,3)[0]=='t',PQgetvalue(grant,0,4)[0]=='t');
  if(!ctw_run(v->state,v->stop_requested,matching,started,allowed,nullable(r,0,26)>now)){if(allowed){status=409;why="Claim expired before dispatch.";goto done;}if(cta_event((coax_v_t_CtView){v,1},ERROR,ts(""),ts("error"),ts("Account admission changed before dispatch"),false,(coax_v_t_CtUsage){&u,1},(coax_v_t_CtEvent){&e,1})!=1||!save_view(db,r,v,&e)){goto done;}}
  else {input=strdup(PQgetvalue(r,0,29));char ms[32];num(ms,now);const char *st[]={account,id,ms};if(!input||cta_event((coax_v_t_CtView){v,1},START,ts(""),ts(""),ts(""),false,(coax_v_t_CtUsage){&u,1},(coax_v_t_CtEvent){&e,1})!=1||!save_view(db,r,v,&e)||!exec_sql(db,"UPDATE chat.turns_v2 SET dispatched_ms=$3::bigint WHERE account=$1 AND id=$2",3,st)){goto done;}}
 }else if(native&&!ct_terminal(v->state)){input=strdup(PQgetvalue(r,0,29));if(!input)goto done;}
 if(!exec_sql(db,"COMMIT",0,NULL)){goto done;}tx=false;
 if(input){model_run(q,account,id,dispatch,worker,key,input);goto finish;}
 PQclear(r);r=turn_row(db,account,id,false);int64_t n=control(r,v,out,CT_CAP);if(n<1){goto done;}reply(q,200,out,(size_t)n);goto finish;
 done: if(tx)exec_sql(db,"ROLLBACK",0,NULL);refuse(q,status,why);
 finish: free(input);free(out);free(v);PQclear(grant);PQclear(r);PQclear(owner);
}
static void runner(const request *q,PGconn *db);
static void runner(const request *q,PGconn *db){
 if(strcmp(q->method,"POST")){refuse(q,405,"POST this");return;}char key[65];int auth=chat_runner_who(q,db,key);if(auth){refuse(q,auth,"Sign this request with a configured runner key.");return;}
 coax_t_CtWorker r;unsigned char arena[232];if(!cwr_read(tn(q->body,q->body_len),(coax_v_t_CtWorker){&r,1},tb(arena,sizeof arena))){refuse(q,400,"Invalid durable worker request.");return;}char op[81],worker[81],id[37],fence[37];copy_string(op,sizeof op,r.id);copy_string(worker,sizeof worker,r.worker);copy_string(id,sizeof id,r.turn);copy_string(fence,sizeof fence,r.dispatch);
 if(r.operation==1)claim(q,db,key,op,worker);else dispatch_turn(q,db,key,id,fence,worker,r.operation==3);
}
bool account_chat_turn_route(const request *q){
 const char *path=q->path;int op=!strcmp(path,"/api/chat/submit/v2alpha1")?1:!strcmp(path,"/api/chat/stop/v2alpha1")?2:!strcmp(path,"/api/chat/turn/v2alpha1")?3:!strcmp(path,"/api/chat/events/v2alpha1")?4:!strcmp(path,"/api/chat/conversations/v2alpha1")?5:!strcmp(path,"/api/chat/conversation/v2alpha1")?6:!strcmp(path,"/api/chat/runner/v2alpha1")?7:!strcmp(path,"/api/chat/context/v2alpha1")?8:!strcmp(path,"/api/chat/input/v2alpha1")?9:!strcmp(path,"/api/chat/native-input/v1alpha1")?10:!strcmp(path,"/api/chat/turn-task/v1alpha1")?11:0;if(!op)return false;
 PGconn *db=api_db();if(!db){refuse(q,503,"Chat storage unavailable.");return true;}if(op==7){runner(q,db);return true;}
 const auth_principal *p=auth_principal_of(q);if(!p||!ctw_owns(p->kind,p->level,ts(p->account),ts(p->account))){refuse(q,401,"Log in first.");return true;}
 bool post=op<=2;if(strcmp(q->method,post?"POST":"GET")){refuse(q,405,post?"POST this":"GET this");return true;}if(post&&!auth_same_site(q)){refuse(q,403,"Cross-site write refused.");return true;}
 if(op==1)submit(q,db,p,"");else if(op==2)stop_turn(q,db,p);else if(op==3||op==4)read_turn(q,db,p,op==4);else if(op==5)conversation_index(q,db,p);else if(op==6)conversation_read(q,db,p);else read_admitted(q,db,p,op);return true;
}
typedef struct {
 request response;char account[41],turn[37],dispatch[37],worker[81],key[65];
 PGconn *db;coax_t_CtStream *stream;double heartbeat;bool cancelled,unavailable;
} turn_model;
static PGconn *model_db(turn_model *m){if(m->db&&PQstatus(m->db)!=CONNECTION_OK){PQfinish(m->db);m->db=NULL;}if(!m->db)m->db=api_db_new();return m->db;}
static bool model_matches(turn_model *m,PGresult *r){return tuples(r)&&PQntuples(r)==1&&!strcmp(PQgetvalue(r,0,22),m->worker)&&!strcmp(PQgetvalue(r,0,23),m->key)&&!strcmp(PQgetvalue(r,0,25),m->dispatch);}
static bool model_emit(turn_model *m,int kind,coax_string text,coax_string reason,coax_string error,bool usage){
 PGconn *db=model_db(m);if(!db){m->unavailable=true;return false;}bool tx=exec_sql(db,"BEGIN",0,NULL);PGresult *r=NULL;coax_t_CtView *v=calloc(1,sizeof *v);bool ok=false;
 if(!tx||!v){goto finish;}r=turn_row(db,m->account,m->turn,true);if(!model_matches(m,r)||!view(r,0,v)){goto finish;}
 if(ct_terminal(v->state)){ok=true;goto finish;}if((kind==TEXT||kind==USAGE)&&v->stop_requested){m->cancelled=true;goto finish;}
 coax_t_CtUsage u=m->stream?m->stream->usage:(coax_t_CtUsage){.input=-1,.output=-1,.cache_read=-1,.cache_write=-1,.reasoning=-1};coax_t_CtEvent e;
 int result=(int)cta_event((coax_v_t_CtView){v,1},kind,text,reason,error,usage,(coax_v_t_CtUsage){&u,1},(coax_v_t_CtEvent){&e,1});
 if(result==4&&m->stream){(void)cts_fail((coax_v_t_CtStream){m->stream,1},ts("Saved reply exceeded its byte limit"));}
 if(result==1&&save_view(db,r,v,&e)&&exec_sql(db,"COMMIT",0,NULL)){tx=false;ok=true;}
 finish: if(tx){exec_sql(db,"ROLLBACK",0,NULL);}PQclear(r);free(v);if(!ok&&!m->cancelled&&(!m->stream||m->stream->bad.count==0))m->unavailable=true;return ok;
}
static bool model_keep(void *opaque){
 turn_model *m=opaque;double now=api_now_ms();if(now-m->heartbeat<1500)return !m->cancelled&&!m->unavailable;m->heartbeat=now;
 PGconn *db=model_db(m);if(!db){m->unavailable=true;return false;}PGresult *r=turn_row(db,m->account,m->turn,false);bool ok=model_matches(m,r)&&!ct_terminal(number(r,0,3));if(ok&&PQgetvalue(r,0,4)[0]=='t'){m->cancelled=true;ok=false;}
 if(ok){char ms[32];num(ms,(long long)now);const char *v[]={m->account,m->turn,m->dispatch,ms};ok=exec_sql(db,"UPDATE chat.turns_v2 SET heartbeat_ms=$4::bigint WHERE account=$1 AND id=$2 AND dispatch=$3 AND state=2",4,v);}PQclear(r);if(!ok&&!m->cancelled)m->unavailable=true;return ok;
}
static bool model_bytes(void *opaque,int status,const char *data,size_t size){
 turn_model *m=opaque;if(status!=200)return true;int64_t at=0;unsigned char delta[16384];
 while(at<(int64_t)size){int64_t next=cts_feed((coax_v_t_CtStream){m->stream,1},tn(data,(int64_t)size),at);if(next<=at)return false;at=next;
  for(int i=0;i<m->stream->n;i++){if(cts_kind((coax_v_t_CtStream){m->stream,1},i)!=TEXT)continue;int64_t offset=0;for(;;){int64_t n=cts_text((coax_v_t_CtStream){m->stream,1},i,offset,tb(delta,sizeof delta));if(n<0)return false;if(n==0)break;if(!model_emit(m,TEXT,tn(delta,n),ts(""),ts(""),false))return false;offset+=n;}}
  coax_t_CtUsage usage;if(cts_take_usage((coax_v_t_CtStream){m->stream,1},(coax_v_t_CtUsage){&usage,1})&&!model_emit(m,USAGE,ts(""),ts(""),ts(""),true))return false;
 }return true;
}
static void model_end(void *opaque,int status,bool clean,double taken){
 (void)taken;turn_model *m=opaque;int state=m->stream?(int)cts_end((coax_v_t_CtStream){m->stream,1},clean):FAILED;int kind=ERROR;coax_string reason=ts("error"),error=ts("Model request was refused before completion");
 if(m->cancelled){kind=CANCEL;reason=ts("cancelled");error=ts("");}
 else if(status==200&&state==DONE&&!m->unavailable){kind=END;reason=cts_reason((coax_v_t_CtStream){m->stream,1});error=ts("");}
 else if(m->unavailable||status==0||(status==200&&state==UNKNOWN)){kind=LOST;reason=ts("unknown");error=ts("Model outcome is unknown after an interrupted request");}
 else if(m->stream&&m->stream->bad.count>0){error=m->stream->bad;}
 (void)model_emit(m,kind,ts(""),reason,error,true);
 PGconn *db=model_db(m);PGresult *r=db?turn_row(db,m->account,m->turn,false):NULL;coax_t_CtView *v=calloc(1,sizeof *v);unsigned char out[4096];int64_t n=v&&model_matches(m,r)?control(r,v,out,sizeof out):-1;
 // control writes into its documented upper bound; use a small owned buffer.
 if(n>0&&n<=(int64_t)sizeof out)reply(&m->response,200,out,(size_t)n);else refuse(&m->response,503,"Reopen the saved turn to reconcile its outcome.");
 free(v);PQclear(r);if(m->db){PQfinish(m->db);m->db=NULL;}
}
static void hosted_model_run(const request *q,const char *account,const char *turn,const char *dispatch,const char *worker,const char *key,const char *input){
 turn_model m={.response=*q};snprintf(m.account,sizeof m.account,"%s",account);snprintf(m.turn,sizeof m.turn,"%s",turn);snprintf(m.dispatch,sizeof m.dispatch,"%s",dispatch);snprintf(m.worker,sizeof m.worker,"%s",worker);snprintf(m.key,sizeof m.key,"%s",key);m.stream=calloc(1,sizeof *m.stream);
 if(!m.stream){model_end(&m,503,true,0);return;}cts_init((coax_v_t_CtStream){m.stream,1});char subject[128],who[65]={0};snprintf(subject,sizeof subject,"openagents.account-chat.v1:%s",account);(void)pr_hash(ts(subject),tb(who,64));
 inference_observer observer={.bytes=model_bytes,.keep=model_keep,.end=model_end};request next=*q;next.body=input;next.body_len=strlen(input);
 inference_ask_observed(&next,who,true,&observer,&m);
 // The child owns its copied parser/connection until the HTTP result ends.
 if(m.db){PQfinish(m.db);}
 free(m.stream);
}
#include "account-chat-tasks.c"
#include "gamebuilder-agents.c"
