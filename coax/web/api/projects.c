// Platform adapter only: parameterized rows, transactions, random IDs and storage.
// Admission is projects-policy.coax; every owned JSON body uses api-wire.coax.
#include "api.h"
#include "auth-internal.h"
#include "projects-wire.generated.h"
#include "account-chat-wire.generated.h"
#include <openssl/rand.h>
#include <stdlib.h>
#include <string.h>
#define PR_ROOT "/api/projects/v1"
#define PR_BUF (1u<<20)
static coax_string prs(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 prb(void *p,size_t n) {return (coax_v_u8){p,(int64_t)n};}
static bool pf(void *p,size_t cap,int64_t *n,const char *s) {size_t k=strlen(s);if(k>cap)return false;memcpy(p,s,k);*n=(int64_t)k;return true;}
#define PF(p,f,n,s) pf((p)->f.e,sizeof (p)->f.e,&(p)->n,(s))
static bool rows(PGresult *r) {return r && PQresultStatus(r)==PGRES_TUPLES_OK;}
static bool run(PGconn *db,const char *sql,int n,const char *const *v) {PGresult *r=PQexecParams(db,sql,n,NULL,v,NULL,NULL,0);bool ok=r && (PQresultStatus(r)==PGRES_COMMAND_OK || PQresultStatus(r)==PGRES_TUPLES_OK);PQclear(r);return ok;}
static void respond(const request *q,int status,const char *body,size_t n) {api_respond(q->fd,status,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",body,n);}
static void problem(const request *q,int status,const char *code,const char *message,long long revision,const char *op) {unsigned char b[4096];int64_t n=pr_error(prs(code),prs(message),revision,prs(op),prb(b,sizeof b));respond(q,status,(const char *)b,n>0?(size_t)n:0);}
static void digest(const char *body,size_t n,char out[65]) {(void)pr_hash((coax_string){body,(int64_t)n},prb(out,64));out[64]=0;}
bool api_object_id(char out[37],const char *prefix) {unsigned char r[16];if(RAND_bytes(r,16)!=1)return false;memcpy(out,prefix,4);for(int i=0;i<16;i++)snprintf(out+4+2*i,3,"%02x",r[i]);return true;}
static void integer(char out[32],long long value) {snprintf(out,32,"%lld",value);}
static PGresult *project_rows(PGconn *db,const char *account,const char *id,bool lock) {
 const char *v[]={account,id};return PQexecParams(db,lock?
 "SELECT id,account,name,description,instructions,revision,instructions_revision,created_ms,updated_ms,(SELECT count(*) FROM projects.knowledge_v1 k WHERE k.project=p.id),(SELECT count(*) FROM projects.thread_links_v1 l WHERE l.project=p.id AND l.account=p.account)+(SELECT count(*) FROM chat.conversations_v2 c WHERE c.project=p.id AND c.account=p.account),instructions_updated_ms FROM projects.private_v1 p WHERE account=$1 AND id=$2 FOR UPDATE OF p":
 "SELECT id,account,name,description,instructions,revision,instructions_revision,created_ms,updated_ms,(SELECT count(*) FROM projects.knowledge_v1 k WHERE k.project=p.id),(SELECT count(*) FROM projects.thread_links_v1 l WHERE l.project=p.id AND l.account=p.account)+(SELECT count(*) FROM chat.conversations_v2 c WHERE c.project=p.id AND c.account=p.account),instructions_updated_ms FROM projects.private_v1 p WHERE account=$1 AND id=$2",2,NULL,v,NULL,NULL,0);
}
static bool load_project(PGresult *r,int row,coax_t_PrProject *p) {
 if(!rows(r) || row>=PQntuples(r))return false;
 if(!PF(p,id,ni,PQgetvalue(r,row,0)) || !PF(p,owner,no,PQgetvalue(r,row,1)) || !PF(p,name,nn,PQgetvalue(r,row,2)) || !PF(p,description,nd,PQgetvalue(r,row,3)) || !PF(p,instructions,nt,PQgetvalue(r,row,4)))return false;
 p->revision=atoll(PQgetvalue(r,row,5));p->instructions_revision=atoll(PQgetvalue(r,row,6));p->created=atoll(PQgetvalue(r,row,7));p->updated=atoll(PQgetvalue(r,row,8));p->knowledge=atoll(PQgetvalue(r,row,9));p->chats=atoll(PQgetvalue(r,row,10));p->instructions_updated=atoll(PQgetvalue(r,row,11));return true;
}
static bool load_knowledge(PGresult *r,int row,const char *account,coax_t_PrKnowledge *k) {
 if(!rows(r) || row>=PQntuples(r))return false;
 if(!PF(k,id,ni,PQgetvalue(r,row,0)) || !PF(k,project,np,PQgetvalue(r,row,1)) || !PF(k,title,nn,PQgetvalue(r,row,2)) || !PF(k,text,nt,PQgetvalue(r,row,3)) || !PF(k,file,nf,PQgetvalue(r,row,4)) || !PF(k,owner,no,account))return false;
 k->selected=PQgetvalue(r,row,5)[0]=='t';k->revision=atoll(PQgetvalue(r,row,6));k->created=atoll(PQgetvalue(r,row,7));k->updated=atoll(PQgetvalue(r,row,8));return true;
}
static PGresult *knowledge_rows(PGconn *db,const char *id) {return PQexecParams(db,"SELECT id,project,title,body,file_id,selected,revision,created_ms,updated_ms FROM projects.knowledge_v1 WHERE project=$1 ORDER BY id",1,NULL,&id,NULL,NULL,0);}
static bool receipt(PGconn *db,const char *account,const char *operation,const char *path,const char *hash,const char *state,int status,const char *reply,long long now) {
 char st[32],ms[32];integer(st,status);integer(ms,now);const char *v[]={account,operation,path,hash,state,st,reply,ms};return run(db,"INSERT INTO projects.operations_v1(account,operation,path,digest,state,status,reply,created_ms) VALUES($1,$2,$3,$4,$5,$6::int,$7,$8::bigint)",8,v);
}
static bool project_context_from(PGconn *db,const char *account,const char *id,coax_t_PrProject *p,const char *conversation,const char *operation,long long now,project_context *out) {
 PGresult *kr=knowledge_rows(db,id);bool ok=rows(kr) && PQntuples(kr)<=200;int count=0;
 coax_string texts[200],titles[200],sources[200];char *owned[200]={0};
 coax_t_PrKnowledge *k=calloc(1,sizeof *k);unsigned char *kb=calloc(1,131072),*reference=calloc(1,32769),*snapshot=calloc(1,131072);
 ok=ok && k && kb && reference && snapshot;
 for(int i=0;ok && i<PQntuples(kr);i++){if(PQgetvalue(kr,i,5)[0]!='t')continue;memset(k,0,sizeof *k);ok=load_knowledge(kr,i,account,k);int64_t n=ok?pr_knowledge_write((coax_v_t_PrKnowledge){k,1},prb(kb,131071)):-1;
  if(n<1){ok=false;break;} // Retain the object only: the list/context writer owns JSON composition.
  int64_t raw_n=pr_knowledge_object((coax_v_t_PrKnowledge){k,1},prb(kb,131071));if(raw_n<1){ok=false;break;}kb[raw_n]=0;
  owned[count]=strdup((const char *)kb);ok=owned[count]!=NULL;if(!ok)break;texts[count]=prs(PQgetvalue(kr,i,3));titles[count]=prs(PQgetvalue(kr,i,2));sources[count]=prs(owned[count]);count++;
 }
 int64_t rn=ok?pr_reference((coax_v_string){texts,count},(coax_v_string){titles,count},prb(reference,32768)):-1;
 ok=ok && rn>=0 && prp_context(p->nt+rn);char snapshot_id[37]={0};ok=ok && api_object_id(snapshot_id,"ctx_");
 int64_t sn=ok?pr_snapshot((coax_v_t_PrProject){p,1},prs(snapshot_id),prs(conversation),prs(operation),(coax_v_string){sources,count},now,prb(snapshot,131071)):-1;
 char *system=calloc(1,(size_t)p->nt+1);if(system)memcpy(system,p->instructions.e,(size_t)p->nt);else ok=false;
 ok=ok && sn>0;
 if(ok){out->instructions=system;out->reference=(char *)reference;out->snapshot=(char *)snapshot;memcpy(out->snapshot_id,snapshot_id,37);out->revision=p->revision;}
 else {free(system);free(reference);free(snapshot);}
 for(int i=0;i<count;i++){free(owned[i]);}
 free(k);free(kb);PQclear(kr);return ok;
}
void projects_context_free(project_context *c) {free(c->instructions);free(c->reference);free(c->snapshot);memset(c,0,sizeof *c);}
int projects_context(PGconn *db,const char *account,const char *id,const char *conversation,const char *operation,long long revision,long long now,project_context *out) {
 coax_t_PrProject *p=calloc(1,sizeof *p);if(!p)return 503;PGresult *r=project_rows(db,account,id,true);bool ok=load_project(r,0,p);PQclear(r);
 int status=!ok?404:!prp_revision(revision,p->revision)?409:project_context_from(db,account,id,p,conversation,operation,now,out)?200:413;free(p);return status;
}
// Called inside the inference child, before exposing a successful answer.
// The existing metered inference route is the only model caller.
typedef struct {char account[81],project[37],thread[25],operation[81];long long revision;char *record;} project_answer;
static bool save_answer(void *opaque,int status,const char *body,size_t size) {
 project_answer *a=opaque;PGconn *db=api_db_new();if(!db)return false;
 unsigned char *record=calloc(1,131072);char *reply=calloc(1,size+1);if(!record || !reply){free(record);free(reply);PQfinish(db);return false;}memcpy(reply,body,size);
 int64_t n=status==200?act_project_answer(prs(a->record),prs(reply),prb(record,131071)):-1;
 bool ok=run(db,"BEGIN",0,NULL);char revision[32];integer(revision,a->revision);const char *v[]={a->account,a->thread,(const char *)record,revision,a->project};
 PGresult *r=NULL;if(ok && n>0)r=PQexecParams(db,"UPDATE chat.account_threads_v1 t SET record=$3,revision=revision+1,updated_ms=(extract(epoch FROM clock_timestamp())*1000)::bigint WHERE account=$1 AND id=$2 AND revision=$4::bigint AND EXISTS(SELECT 1 FROM projects.thread_links_v1 l JOIN projects.private_v1 p ON p.id=l.project WHERE l.account=$1 AND l.thread=$2 AND l.project=$5 AND p.account=$1) RETURNING id",5,NULL,v,NULL,NULL,0);
 ok=ok && n>0 && rows(r) && PQntuples(r)==1;PQclear(r);
 const char *done[]={a->account,a->operation,ok?reply:"",ok?"200":"409",ok?"done":"failed"};
 if(ok)ok=run(db,"UPDATE projects.operations_v1 SET reply=$3,status=$4::int,state=$5 WHERE account=$1 AND operation=$2",5,done);
 if(ok)ok=run(db,"COMMIT",0,NULL);else {run(db,"ROLLBACK",0,NULL);run(db,"UPDATE projects.operations_v1 SET reply='',status=409,state='failed' WHERE account=$1 AND operation=$2",2,done);}
 free(record);free(reply);PQfinish(db);return ok;
}
static bool complete(const request *q,PGconn *db,const auth_principal *actor,coax_t_PrProject *p,coax_t_PrRequest *req,const char *operation,const char *hash) {
 char thread[25]={0};memcpy(thread,req->thread.e,(size_t)req->nthread);
 char project_id[37]={0};memcpy(project_id,p->id.e,(size_t)p->ni);
 const char *v[]={actor->account,thread,project_id};
 PGresult *tr=PQexecParams(db,"SELECT t.record,t.revision FROM chat.account_threads_v1 t JOIN projects.thread_links_v1 l ON l.account=t.account AND l.thread=t.id WHERE t.account=$1 AND t.id=$2 AND l.project=$3 FOR UPDATE OF t",3,NULL,v,NULL,NULL,0);
 if(!rows(tr) || PQntuples(tr)!=1 || !prp_revision(req->thread_revision,atoll(PQgetvalue(tr,0,1)))) {PQclear(tr);run(db,"ROLLBACK",0,NULL);problem(q,409,"revision_conflict","Reopen the saved Project conversation.",p->revision,operation);return true;}
 if(!act_funded(actor->kind,prs(actor->account),actor->team)){PQclear(tr);run(db,"ROLLBACK",0,NULL);problem(q,403,"forbidden","Agent chat is currently available to the team beta.",p->revision,operation);return true;}
 char turn_revision[32];integer(turn_revision,req->thread_revision);const char *prior[]={actor->account,thread,turn_revision};PGresult *attempt=PQexecParams(db,"SELECT operation FROM projects.contexts_v1 WHERE account=$1 AND thread=$2 AND thread_revision=$3::bigint",3,NULL,prior,NULL,NULL,0);bool available=rows(attempt) && prp_attempt(PQntuples(attempt)>0);PQclear(attempt);if(!available){PQclear(tr);run(db,"ROLLBACK",0,NULL);problem(q,409,"attempt_exists","This saved turn already has a model attempt. Reopen the conversation to check its result.",p->revision,operation);return true;}
 project_context context={0};long long now=(long long)api_now_ms();
 bool ok=project_context_from(db,actor->account,project_id,p,thread,operation,now,&context);
 unsigned char *request_body=calloc(1,262144);ok=ok && request_body;
 int64_t bn=ok?act_project_completion(prs(PQgetvalue(tr,0,0)),prs(context.instructions),prs(context.reference),prb(request_body,262143)):-1;
 project_answer a={0};snprintf(a.account,sizeof a.account,"%s",actor->account);snprintf(a.project,sizeof a.project,"%.*s",(int)p->ni,p->id.e);snprintf(a.thread,sizeof a.thread,"%s",thread);snprintf(a.operation,sizeof a.operation,"%s",operation);a.revision=req->thread_revision;a.record=rows(tr)?strdup(PQgetvalue(tr,0,0)):NULL;
 ok=ok && bn>0 && a.record;char rev[32];integer(rev,p->revision);const char *sv[]={actor->account,project_id,operation,thread,rev,turn_revision,context.snapshot,(const char *)request_body};
 if(ok)ok=receipt(db,actor->account,operation,q->path,hash,"running",202,"",now) && run(db,"INSERT INTO projects.contexts_v1(account,project,operation,thread,revision,thread_revision,snapshot,input_json) VALUES($1,$2,$3,$4,$5::bigint,$6::bigint,$7,$8)",8,sv) && run(db,"COMMIT",0,NULL);
 if(!ok)run(db,"ROLLBACK",0,NULL);
 projects_context_free(&context);PQclear(tr);
 if(!ok){free(a.record);free(request_body);problem(q,413,"limit_exceeded","The complete Project context does not fit. Remove knowledge or start a shorter conversation.",p->revision,operation);return true;}
 char subject[128],who[65];snprintf(subject,sizeof subject,"openagents.account-chat.v1:%s",actor->account);digest(subject,strlen(subject),who);
 request next=*q;next.body=(const char *)request_body;next.body_len=(size_t)bn;
 bool result=inference_ask_saved(&next,who,true,save_answer,&a);free(a.record);free(request_body);return result;
}

bool projects_route(const request *q) {
 // A legacy project may have the slug v1; its attachment route keeps its owner.
 if(!strcmp(q->path,PR_ROOT "/files"))return false;
 size_t root=strlen(PR_ROOT);if(strncmp(q->path,PR_ROOT,root) || (q->path[root] && q->path[root]!='/'))return false;
 const auth_principal *actor=auth_principal_of(q);
 if(!actor || !prp_access(actor->kind,actor->level>=1,prs(actor->account),prs(actor->account))){problem(q,401,"unauthorized","Log in first.",-1,"");return true;}
 PGconn *db=api_db();if(!db){problem(q,503,"unavailable","Project storage unavailable.",-1,"");return true;}
 const char *rest=q->path+root;while(*rest=='/')rest++;char id[37]={0},kid[37]={0},tid[25]={0};const char *op="";
 if(*rest){const char *slash=strchr(rest,'/');size_t n=slash?(size_t)(slash-rest):strlen(rest);if(n==36)memcpy(id,rest,36);if(!prp_id(prs(id))){problem(q,404,"not_found","not found",-1,"");return true;}op=slash?slash+1:"";}
 bool post=!strcmp(q->method,"POST");if(!post && strcmp(q->method,"GET")){problem(q,405,"invalid_argument","GET or POST this.",-1,"");return true;}
 if(post && !auth_same_site(q)){problem(q,403,"forbidden","Cross-site write refused.",-1,"");return true;}
 unsigned char *out=calloc(1,PR_BUF);coax_t_PrProject *p=calloc(1,sizeof *p);coax_t_PrRequest *req=calloc(1,sizeof *req);coax_t_PrKnowledge *k=calloc(1,sizeof *k);
 if(!out || !p || !req || !k){free(out);free(p);free(req);free(k);problem(q,503,"unavailable","Memory unavailable.",-1,"");return true;}
 const char *err="";int status=200;int64_t used=-1;bool transaction=false,handled=false;char operation[81]={0},hash[65]={0};
 int kind=!*id?1:!strcmp(op,"update")?2:!strcmp(op,"instructions")?3:!strcmp(op,"knowledge")?4:!strncmp(op,"knowledge/",10)?5:!strcmp(op,"chats")?6:!strcmp(op,"complete")?7:0;
 if(!strcmp(op,"chats/new"))kind=8;
 if(!strncmp(op,"chats/",6) && strlen(op+6)>=22){memcpy(tid,op+6,22);if(strlen(op+6)==31 && !strcmp(op+28,"/messages"))kind=9;}
 if(kind==5){const char *start=op+10;if(strlen(start)==43 && !strcmp(start+36,"/remove"))memcpy(kid,start,36);else kind=0;}
 if(post){coax_string raw={(const char *)q->body,(int64_t)q->body_len};if(!kind || !(kind>=8?pr_chat_request(raw,kind==8,(coax_v_t_PrRequest){req,1}):pr_request(raw,kind,(coax_v_t_PrRequest){req,1}))){status=400;err="Invalid Project request.";goto done;}memcpy(operation,req->operation.e,(size_t)req->no);digest(q->body,q->body_len,hash);
  if(!prp_change(true,actor->limited,actor->frozen!=0,false)){status=403;err="This account cannot change Projects.";goto done;}
  transaction=run(db,"BEGIN",0,NULL);if(!transaction){status=503;err="Storage unavailable.";goto done;}
  const char *lock[]={actor->account,operation};if(!run(db,"SELECT pg_advisory_xact_lock(hashtextextended($1||':'||$2,0))",2,lock)){status=503;err="Storage unavailable.";goto done;}
 }
 if(*id){PGresult *r=project_rows(db,actor->account,id,post);bool ok=load_project(r,0,p);if(ok)ok=prp_access(actor->kind,true,prs(actor->account),(coax_string){(const char *)p->owner.e,p->no});PQclear(r);if(!ok){status=404;err="not found";goto done;}}
 if(post){const char *v[]={actor->account,operation};PGresult *r=PQexecParams(db,"SELECT path,digest,state,status,reply FROM projects.operations_v1 WHERE account=$1 AND operation=$2",2,NULL,v,NULL,NULL,0);if(!rows(r)){PQclear(r);status=503;err="Storage unavailable.";goto done;}
  if(PQntuples(r)){if(strcmp(PQgetvalue(r,0,0),q->path) || strcmp(PQgetvalue(r,0,1),hash)){status=409;err="Operation ID already has different contents.";}else if(!strcmp(PQgetvalue(r,0,2),"done")){status=atoi(PQgetvalue(r,0,3));used=(int64_t)strlen(PQgetvalue(r,0,4));if(used<PR_BUF)memcpy(out,PQgetvalue(r,0,4),(size_t)used);else{status=503;used=-1;err="Stored reply too large.";}}else{status=409;err="This model attempt is pending or failed. Reopen the conversation; it will not be dispatched twice.";}PQclear(r);goto done;}PQclear(r);
  if(*id && !prp_revision(req->revision,p->revision)){status=409;err="Project changed. Reopen it before saving.";goto done;}
 }
 if(!post && !*id){char cursor[37]={0};api_query(q,"cursor",cursor,sizeof cursor);const char *v[]={actor->account,cursor};PGresult *r=PQexecParams(db,"SELECT id FROM projects.private_v1 WHERE account=$1 AND id>$2 ORDER BY id LIMIT 33",2,NULL,v,NULL,NULL,0);coax_string records[32];char *owned[32]={0};int count=0;bool ok=rows(r);char next[37]={0};
  for(int i=0;ok && i<PQntuples(r) && i<32;i++){PGresult *one=project_rows(db,actor->account,PQgetvalue(r,i,0),false);ok=load_project(one,0,p);PQclear(one);int64_t n=ok?pr_summary((coax_v_t_PrProject){p,1},prb(out,PR_BUF-1)):-1;if(n<1){ok=false;break;}out[n]=0;owned[count]=strdup((const char *)out);ok=owned[count]!=NULL;if(ok){records[count]=prs(owned[count]);count++;}}
  if(ok && PQntuples(r)>32 && count>0)snprintf(next,sizeof next,"%s",PQgetvalue(r,31,0));PQclear(r);used=ok?pr_records((coax_v_string){records,count},-1,prs(next),prb(out,PR_BUF-1)):-1;for(int i=0;i<count;i++)free(owned[i]);goto done;
 }
 if(!post && !strcmp(op,"instructions")){used=pr_instructions_write((coax_v_t_PrProject){p,1},prb(out,PR_BUF-1));goto done;}
 if(!post && !*op){used=pr_project_write((coax_v_t_PrProject){p,1},prb(out,PR_BUF-1));goto done;}
 if(!post && *tid && strlen(op+6)==22){const char *v[]={actor->account,id,tid};PGresult *r=PQexecParams(db,"SELECT t.record FROM chat.account_threads_v1 t JOIN projects.thread_links_v1 l ON l.account=t.account AND l.thread=t.id WHERE l.account=$1 AND l.project=$2 AND l.thread=$3",3,NULL,v,NULL,NULL,0);if(rows(r) && PQntuples(r)==1){used=(int64_t)strlen(PQgetvalue(r,0,0));memcpy(out,PQgetvalue(r,0,0),(size_t)used);}else{status=404;err="Chat not found in this Project.";}PQclear(r);goto done;}
 if(!post && !strcmp(op,"chats")){const char *v[]={actor->account,id};PGresult *r=PQexecParams(db,"SELECT t.account,t.id,t.title,'google/gemini-4-hydrogen',l.project,t.updated_ms::text,'false' FROM chat.account_threads_v1 t JOIN projects.thread_links_v1 l ON l.account=t.account AND l.thread=t.id WHERE l.account=$1 AND l.project=$2 ORDER BY t.updated_ms DESC,t.id LIMIT 60",2,NULL,v,NULL,NULL,0);if(!rows(r)){PQclear(r);status=503;err="Chats unavailable.";goto done;}handled=chat_history_rows(q,r);goto done;}
 if(!post && !strcmp(op,"context")){char key[81]={0};api_query(q,"operation_id",key,sizeof key);const char *v[]={actor->account,id,key};PGresult *r=PQexecParams(db,"SELECT snapshot FROM projects.contexts_v1 WHERE account=$1 AND project=$2 AND operation=$3",3,NULL,v,NULL,NULL,0);if(rows(r) && PQntuples(r)==1){used=(int64_t)strlen(PQgetvalue(r,0,0));if(used<PR_BUF)memcpy(out,PQgetvalue(r,0,0),(size_t)used);else used=-1;}else{status=404;err="not found";}PQclear(r);goto done;}
 if(!post && !strncmp(op,"knowledge/",10) && strlen(op+10)==44 && !strcmp(op+46,"/content")){char key[37]={0};memcpy(key,op+10,36);const char *v[]={id,key};PGresult *r=PQexecParams(db,"SELECT id,project,title,body,file_id,selected,revision,created_ms,updated_ms FROM projects.knowledge_v1 WHERE project=$1 AND id=$2",2,NULL,v,NULL,NULL,0);if(load_knowledge(r,0,actor->account,k))used=pr_knowledge_content((coax_v_t_PrKnowledge){k,1},prb(out,PR_BUF-1));else{status=404;err="not found";}PQclear(r);goto done;}
 if(!post && !strcmp(op,"knowledge")){PGresult *r=knowledge_rows(db,id);coax_string records[200];char *owned[200]={0};int count=0;bool ok=rows(r) && PQntuples(r)<=200;
  for(int i=0;ok && i<PQntuples(r);i++){memset(k,0,sizeof *k);ok=load_knowledge(r,i,actor->account,k);int64_t n=ok?pr_knowledge_object((coax_v_t_PrKnowledge){k,1},prb(out,PR_BUF-1)):-1;if(n<1){ok=false;break;}out[n]=0;owned[count]=strdup((const char *)out);ok=owned[count]!=NULL;if(ok){records[count]=prs(owned[count]);count++;}}PQclear(r);used=ok?pr_records((coax_v_string){records,count},p->revision,prs(""),prb(out,PR_BUF-1)):-1;for(int i=0;i<count;i++)free(owned[i]);goto done;
 }
 if(!post){status=404;err="not found";goto done;}
 long long now=(long long)api_now_ms();char ms[32],rev[32];integer(ms,now);integer(rev,p->revision+1);
 if(kind==1){if(!api_object_id(id,"prj_")){status=503;err="ID unavailable.";goto done;}PF(p,id,ni,id);PF(p,owner,no,actor->account);memcpy(p->name.e,req->name.e,(size_t)req->nn);p->nn=req->nn;memcpy(p->description.e,req->description.e,(size_t)req->nd);p->nd=req->nd;memcpy(p->instructions.e,req->text.e,(size_t)req->nt);p->nt=req->nt;p->revision=1;p->instructions_revision=1;p->instructions_updated=now;p->created=now;p->updated=now;
  char name[161]={0},desc[4097]={0},text[65537]={0};memcpy(name,p->name.e,(size_t)p->nn);memcpy(desc,p->description.e,(size_t)p->nd);memcpy(text,p->instructions.e,(size_t)p->nt);const char *v[]={id,actor->account,name,desc,text,ms};if(!run(db,"INSERT INTO projects.private_v1(id,account,name,description,instructions,revision,instructions_revision,instructions_updated_ms,created_ms,updated_ms) VALUES($1,$2,$3,$4,$5,1,1,$6::bigint,$6::bigint,$6::bigint)",6,v)){status=503;err="Project could not be saved.";goto done;}status=201;
 }else if(kind==2 || kind==3){char name[161]={0},desc[4097]={0},text[65537]={0};if(req->nn>=0){memcpy(p->name.e,req->name.e,(size_t)req->nn);p->nn=req->nn;}if(req->nd>=0){memcpy(p->description.e,req->description.e,(size_t)req->nd);p->nd=req->nd;}if(kind==3){memcpy(p->instructions.e,req->text.e,(size_t)req->nt);p->nt=req->nt;p->instructions_revision=p->revision+1;p->instructions_updated=now;}memcpy(name,p->name.e,(size_t)p->nn);memcpy(desc,p->description.e,(size_t)p->nd);memcpy(text,p->instructions.e,(size_t)p->nt);char ir[32];integer(ir,p->instructions_revision);const char *v[]={id,name,desc,text,rev,ir,ms};if(!run(db,"UPDATE projects.private_v1 SET name=$2,description=$3,instructions=$4,revision=$5::bigint,instructions_revision=$6::bigint,updated_ms=$7::bigint,instructions_updated_ms=CASE WHEN instructions_revision=$6::bigint THEN instructions_updated_ms ELSE $7::bigint END WHERE id=$1",7,v)){status=503;err="Project could not be saved.";goto done;}p->revision++;p->updated=now;
 }else if(kind==4){if(req->nf>0){char fid[25]={0},subject[65]={0};memcpy(fid,req->file.e,(size_t)req->nf);if(!project_subject(q,db,subject)){status=503;err="Upload identity unavailable.";goto done;}
   const char *v[]={fid,subject,actor->pubkey};PGresult *r=PQexecParams(db,"SELECT d.owner,f.status,f.ext,f.bytes,f.object FROM drops.files f JOIN drops.drops d ON d.id=f.drop_id WHERE f.id=$1 AND d.owner IN($2,$3)",3,NULL,v,NULL,NULL,0);
   bool ok=rows(r) && PQntuples(r)==1 && prp_file(true,!strcmp(PQgetvalue(r,0,1),"scanned"),prs(PQgetvalue(r,0,2)),atoll(PQgetvalue(r,0,3)));size_t n=0;unsigned char *bytes=ok?drops_fetch(PQgetvalue(r,0,4),65536,&n):NULL;ok=ok && bytes && n==(size_t)atoll(PQgetvalue(r,0,3)) && pr_text_valid((coax_string){(const char *)bytes,(int64_t)n});if(ok){memcpy(req->text.e,bytes,n);req->nt=(int64_t)n;}free(bytes);PQclear(r);if(!ok){status=422;err="Choose an owned scanned UTF-8 text or Markdown file up to 64 KiB.";goto done;}}
  if(!prp_capacity(p->knowledge,req->nt)){status=413;err="Project knowledge is full.";goto done;}char ki[37];if(!api_object_id(ki,"knw_")){status=503;err="ID unavailable.";goto done;}PF(k,id,ni,ki);PF(k,project,np,id);PF(k,owner,no,actor->account);memcpy(k->title.e,req->title.e,(size_t)req->nl);k->nn=req->nl;memcpy(k->text.e,req->text.e,(size_t)req->nt);k->nt=req->nt;memcpy(k->file.e,req->file.e,(size_t)req->nf);k->nf=req->nf;k->selected=req->selected;k->revision=p->revision+1;k->created=now;k->updated=now;
  char title[257]={0},body[65537]={0},file[25]={0};memcpy(title,k->title.e,(size_t)k->nn);memcpy(body,k->text.e,(size_t)k->nt);memcpy(file,k->file.e,(size_t)k->nf);const char *v[]={ki,id,title,body,file,k->selected?"true":"false",rev,ms};if(!run(db,"INSERT INTO projects.knowledge_v1(id,project,title,body,file_id,selected,revision,created_ms,updated_ms) VALUES($1,$2,$3,$4,$5,$6::bool,$7::bigint,$8::bigint,$8::bigint)",8,v)){status=503;err="Knowledge could not be saved.";goto done;}p->knowledge++;status=201;
 }else if(kind==5){const char *v[]={id,kid};PGresult *r=PQexecParams(db,"DELETE FROM projects.knowledge_v1 WHERE project=$1 AND id=$2 RETURNING id",2,NULL,v,NULL,NULL,0);bool ok=rows(r) && PQntuples(r)==1;PQclear(r);if(!ok){status=404;err="not found";goto done;}p->knowledge--;
 }else if(kind==6){char thread[25]={0};memcpy(thread,req->thread.e,(size_t)req->nthread);const char *v[]={actor->account,thread,id};PGresult *r=PQexecParams(db,"SELECT t.account,coalesce(l.project,old.project,'') FROM chat.account_threads_v1 t LEFT JOIN chat.project_threads_v1 old ON old.account=t.account AND old.thread=t.id LEFT JOIN projects.thread_links_v1 l ON l.account=t.account AND l.thread=t.id WHERE t.account=$1 AND t.id=$2 FOR UPDATE OF t",2,NULL,v,NULL,NULL,0);
  bool linked=rows(r) && PQntuples(r)==1 && *PQgetvalue(r,0,1);bool ok=rows(r) && PQntuples(r)==1 && prp_access(actor->kind,true,prs(actor->account),prs(PQgetvalue(r,0,0))) && (!*PQgetvalue(r,0,1) || !strcmp(PQgetvalue(r,0,1),id));PQclear(r);if(!ok){status=409;err="This conversation cannot be linked to that Project.";goto done;}if(!run(db,"INSERT INTO projects.thread_links_v1(account,thread,project) VALUES($1,$2,$3) ON CONFLICT(account,thread) DO NOTHING",3,v)){status=503;err="Conversation link unavailable.";goto done;} if(!linked)p->chats++; // The association never reinterprets a frozen v1 link.
 }else if(kind==7){handled=complete(q,db,actor,p,req,operation,hash);transaction=false;goto done;
 }else if(kind==8 || kind==9){char text[16385]={0};memcpy(text,req->text.e,(size_t)req->nt);
  if(kind==8){if(!account_thread_id(tid)){status=503;err="ID unavailable.";goto done;}used=act_project_start(prs(tid),prs(text),prb(out,65536));if(used<1){status=413;err="Chat does not fit.";goto done;}out[used]=0;const char *v[]={actor->account,tid,(const char *)out,text,ms};const char *link[]={actor->account,tid,id};if(!run(db,"INSERT INTO chat.account_threads_v1(account,id,record,revision,title,updated_ms) VALUES($1,$2,$3,1,left($4,80),$5::bigint)",5,v) || !run(db,"INSERT INTO projects.thread_links_v1(account,thread,project) VALUES($1,$2,$3)",3,link)){status=503;used=-1;err="Chat could not be saved.";goto done;}p->chats++;status=201;
  }else{const char *v[]={actor->account,tid,id};PGresult *r=PQexecParams(db,"SELECT t.record,t.revision FROM chat.account_threads_v1 t JOIN projects.thread_links_v1 l ON l.account=t.account AND l.thread=t.id WHERE l.account=$1 AND l.project=$3 AND l.thread=$2 FOR UPDATE OF t",3,NULL,v,NULL,NULL,0);if(!rows(r) || PQntuples(r)!=1){PQclear(r);status=404;err="Chat not found in this Project.";goto done;}if(!prp_revision(req->thread_revision,atoll(PQgetvalue(r,0,1)))){PQclear(r);status=409;err="Chat changed. Reopen it before sending.";goto done;}used=act_project_user(prs(PQgetvalue(r,0,0)),prs(text),prb(out,65536));PQclear(r);if(used<1){status=409;err="Chat needs an assistant answer first, or is full.";goto done;}out[used]=0;char tr[32];integer(tr,req->thread_revision+1);const char *sv[]={actor->account,tid,(const char *)out,tr,ms};if(!run(db,"UPDATE chat.account_threads_v1 SET record=$3,revision=$4::bigint,updated_ms=$5::bigint WHERE account=$1 AND id=$2",5,sv)){status=503;used=-1;err="Chat could not be saved.";goto done;}}
 }
 if((kind>=4 && kind<=6) || kind==8){const char *v[]={id,rev,ms};if(!run(db,"UPDATE projects.private_v1 SET revision=$2::bigint,updated_ms=$3::bigint WHERE id=$1",3,v)){status=503;used=-1;err="Project revision unavailable.";goto done;}p->revision++;p->updated=now;}
 if(kind<8)used=kind==3?pr_instructions_write((coax_v_t_PrProject){p,1},prb(out,PR_BUF-1)):kind==4?pr_knowledge_write((coax_v_t_PrKnowledge){k,1},prb(out,PR_BUF-1)):kind==5?pr_action(prs(kid),p->revision,now,prb(out,PR_BUF-1)):pr_project_write((coax_v_t_PrProject){p,1},prb(out,PR_BUF-1));
 if(used>0)out[used]=0;
 if(used>0 && !receipt(db,actor->account,operation,q->path,hash,"done",status,(const char *)out,now)){used=-1;status=503;err="Mutation receipt unavailable.";}
 if(used>0){if(!run(db,"COMMIT",0,NULL)){used=-1;status=503;err="Save result unknown. Retry the same operation ID.";}transaction=false;}
 done:
 if(transaction)run(db,"ROLLBACK",0,NULL);
 if(!handled){if(used>=0)respond(q,status,(const char *)out,(size_t)used);else problem(q,status>=400?status:503,status==404?"not_found":status==409?"revision_conflict":status==403?"forbidden":status==413?"limit_exceeded":status==422?"unsupported_source":status>=500?"unavailable":"invalid_argument",*err?err:"The complete result did not fit.",*id?p->revision:-1,operation);}
 free(out);free(p);free(req);free(k);return true;
}
