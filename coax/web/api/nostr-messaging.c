// Firm reason: 6 (PostgreSQL and authenticated HTTP SDK effects only).
// Coax decodes, admits, verifies and prepares bytes; SQL commits the whole
// journal before this route publishes a successful response.
#include "auth-internal.h"
#include "nostr-messaging.generated.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static coax_string nss(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 nb(void *s,size_t n) {return (coax_v_u8){s,(int64_t)n};}
static bool sql_ok(PGconn *db,const char *sql) {
 PGresult *r=PQexec(db,sql);bool ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);return ok;
}
bool nostr_messaging_route(const request *q,PGconn *db,const char *op) {
 int effect=!strcmp(op,"nostr/store/v1alpha1") ? 1:!strcmp(op,"nostr/custody/v1alpha1") ? 2:!strcmp(op,"nostr/operation/v1alpha1") ? 3:!strcmp(op,"nostr/send/v1alpha1") ? 4:!strcmp(op,"nostr/delivery/v1alpha1") ? 5:!strcmp(op,"nostr/read/v1") ? 6:0;
 if (!effect) return false;
 api_route_at("nostr_messaging_route");
 const auth_principal *p=auth_principal_of(q);
 if (!p || !p->account[0] || !p->session[0]) {api_error(q->fd,401,"Log in first.");return true;}
 if (strcmp(q->method,"POST")) {api_error(q->fd,405,"Use POST.");return true;}
 if (!db) {api_error(q->fd,503,"Store unavailable.");return true;}
 coax_t_MsRequest *input=calloc(1,sizeof *input);coax_t_Nms *state=calloc(1,sizeof *state);
 coax_t_NscRequest *custody=calloc(1,sizeof *custody);coax_t_NsoRequest *receipt=calloc(1,sizeof *receipt);coax_t_NscPurpose *purpose=calloc(1,sizeof *purpose);coax_t_Nip59Fresh *fresh=calloc(11,sizeof *fresh);
 coax_t_NsmHeads *heads=calloc(1,sizeof *heads);coax_t_NsmSend *batch=calloc(1,sizeof *batch);unsigned char *send_body=calloc(1,1048576);
 coax_t_NrdMarker *read_marker=calloc(1,sizeof *read_marker);
 coax_t_NdlRequest *delivery=calloc(1,sizeof *delivery);coax_t_NdlPage *delivery_page=calloc(1,sizeof *delivery_page);
 coax_t_RsdRec *records=calloc(512,sizeof *records);
 unsigned char *document=calloc(1,8388608),*replacement=calloc(1,8388608),*arena=calloc(1,8388608),*payload=calloc(1,65537),*response=calloc(1,400000),*head=calloc(1,65536);
 unsigned char secret[32]={0};char digest[65]={0},operation[65]={0};int64_t source=0;bool save_receipt=false;coax_t_NscEffect crypto={0};
 bool txn=false;int error=503;const char *message="Store unavailable.";coax_t_NmsChange result={0};
 if (!input || !state || !records || !document || !replacement || !arena || !payload || !response || !custody || !receipt || !purpose || !fresh || !head || !heads || !batch || !send_body || !delivery || !delivery_page || !read_marker) goto done;
 coax_v_t_MsRequest iv={input,1};coax_v_t_Nms sv={state,1};
 coax_v_t_NscRequest cv={custody,1};coax_v_t_NsoRequest ov={receipt,1};coax_v_t_NsmSend bv={batch,1};coax_v_t_NsmHeads hv={heads,1};coax_string body={q->body,(int64_t)q->body_len};
 if (q->body_len>(effect==4 ? 1048576:400000) || !(effect==1 ? nms_request_read(body,iv):effect==2 ? nsc_read(body,cv):effect==3 ? nso_read(body,ov):effect==4 ? nsm_send_read(body,bv):effect==6 ? nrd_read(body,false,(coax_v_t_NrdMarker){read_marker,1}):ndl_request_read(body,(coax_v_t_NdlRequest){delivery,1}))) {error=400;message="Invalid Nostr request.";goto done;}
 if (effect==4) {receipt->action=3;memcpy(receipt->identity.e,batch->identity.e,64);memcpy(receipt->operation.e,batch->operation.e,64);memcpy(receipt->digest.e,batch->digest.e,64);}
 char expected[65]={0},canonical[65]={0},scope_hex[65]={0},origin[128]={0};unsigned char scope[32];
 memcpy(expected,effect==1 ? input->identity.e:effect==2 ? custody->identity.e:effect==5 ? delivery->identity.e:effect==6 ? read_marker->identity.e:receipt->identity.e,64);
 coax_t_MsFacts facts={.authorized=asp_write(p->kind,true,p->limited,p->frozen),.fresh=asp_backup(p->kind,true,p->limited,p->frozen,p->level,p->l2_age_ms)};
 if (!facts.authorized) {error=403;message="Verify your account first.";goto done;}
 int auth_source=auth_identity_ensure(db,p->account,canonical);if (auth_source<0) goto done;
 facts.matched=!strcmp(expected,canonical);
 if (!facts.matched) {result.status=3;goto answer;}
 if (!nms_scope(nss(p->account),nss(canonical),nb(scope,32))) goto done;
 auth_hex(scope,32,scope_hex);snprintf(origin,sizeof origin,"%s",p->account);
 if (!sql_ok(db,"BEGIN")) goto done;txn=true;
 // The account lock also serializes explicit account lifecycle operations.
 const char *owner[]={p->account};PGresult *row=PQexecParams(db,"SELECT id FROM auth.accounts WHERE id=$1 FOR UPDATE",1,NULL,owner,NULL,NULL,0);
 bool ok=PQresultStatus(row)==PGRES_TUPLES_OK && PQntuples(row)==1;PQclear(row);if (!ok) goto done;
 const char *lookup[]={p->account,canonical};
 row=PQexecParams(db,"SELECT namespace,origin_account,revision::text,pruned::text,document FROM auth.nostr_journals_v1alpha1 WHERE account=$1 AND identity=$2 ORDER BY namespace FOR UPDATE",2,NULL,lookup,NULL,NULL,1);
 if (PQresultStatus(row)!=PGRES_TUPLES_OK) {PQclear(row);goto done;}
 // Merged archives are preserved; an ambiguous namespace is never replaced.
 if (PQntuples(row)>1) {PQclear(row);result.status=11;goto answer;}
 int64_t total=0,revision=0;bool pruned=false;
 if (PQntuples(row)==1) {
  total=PQgetlength(row,0,4);if (total<68 || total>8388608 || PQgetlength(row,0,0)!=64 || PQgetlength(row,0,1)>127) {PQclear(row);result.status=10;goto answer;}
  memcpy(scope_hex,PQgetvalue(row,0,0),64);snprintf(origin,sizeof origin,"%s",PQgetvalue(row,0,1));revision=strtoll(PQgetvalue(row,0,2),NULL,10);pruned=!strcmp(PQgetvalue(row,0,3),"true");memcpy(document,PQgetvalue(row,0,4),(size_t)total);
 }
 PQclear(row);
 result.status=nms_mount(sv,nb(document,8388608),total,nss(origin),nss(canonical),revision,pruned,nb(arena,8388608),(coax_v_t_RsdRec){records,512},(int64_t)time(NULL));
 result.revision=revision;result.pruned=pruned;if (result.status) goto answer;
 // The namespace column must match the identity-bound original header too.
 if (!nms_scope(nss(origin),nss(canonical),nb(scope,32))) {result.status=10;goto answer;}
 char actual_scope[65];auth_hex(scope,32,actual_scope);if (strcmp(scope_hex,actual_scope)) {result.status=10;goto answer;}
 if (effect==6) {
  char conversation[65]={0},event[65]={0},previous[65]={0};memcpy(conversation,read_marker->conversation.e,64);memcpy(event,read_marker->event.e,(size_t)read_marker->ne);
  const char *keys[]={scope_hex,p->account,canonical,conversation};
  row=PQexecParams(db,"SELECT created_at::text,event FROM auth.nostr_read_markers_v1 WHERE namespace=$1 AND account=$2 AND identity=$3 AND conversation=$4",4,NULL,keys,NULL,NULL,0);
  if (PQresultStatus(row)!=PGRES_TUPLES_OK || PQntuples(row)>1) {PQclear(row);goto done;}
  bool exists=PQntuples(row)==1;int64_t previous_at= -1;
  if (exists) {previous_at=strtoll(PQgetvalue(row,0,0),NULL,10);if (PQgetlength(row,0,1)!=64) {PQclear(row);goto done;}memcpy(previous,PQgetvalue(row,0,1),64);}PQclear(row);
  if (nrd_later(read_marker->created,nss(event),previous_at,nss(previous))) {
   row=PQexecParams(db,"SELECT count(*)::text FROM auth.nostr_read_markers_v1 WHERE namespace=$1",1,NULL,keys,NULL,NULL,0);
   ok=PQresultStatus(row)==PGRES_TUPLES_OK && PQntuples(row)==1;if (ok) result.status=nrd_mark_status(total>0,strtoll(PQgetvalue(row,0,0),NULL,10),exists);PQclear(row);if (!ok) goto done;if (result.status!=0) goto answer;
   char created[32];snprintf(created,sizeof created,"%lld",(long long)read_marker->created);const char *values[]={scope_hex,p->account,canonical,conversation,created,event};
   row=PQexecParams(db,"INSERT INTO auth.nostr_read_markers_v1(namespace,account,identity,conversation,created_at,event) VALUES($1,$2,$3,$4,$5::bigint,$6) ON CONFLICT(namespace,conversation) DO UPDATE SET created_at=EXCLUDED.created_at,event=EXCLUDED.event",6,NULL,values,NULL,NULL,0);ok=PQresultStatus(row)==PGRES_COMMAND_OK;PQclear(row);if (!ok) goto done;
   previous_at=read_marker->created;memcpy(previous,event,65);
  }
  read_marker->created=previous_at;read_marker->ne=previous_at<0 ? 0:64;memset(read_marker->event.e,0,64);memcpy(read_marker->event.e,previous,(size_t)read_marker->ne);
  result.payload_n=nrd_write((coax_v_t_NrdMarker){read_marker,1},true,nb(payload,65536));if (result.payload_n<0) goto done;
  if (!sql_ok(db,"COMMIT")) goto done;txn=false;goto answer;
 }
 if (effect==5) {
  if (!ndl_begin((coax_v_t_NdlPage){delivery_page,1},(coax_v_t_NdlRequest){delivery,1})) goto done;
  char after[65]={0};memcpy(after,delivery->after.e,(size_t)delivery->na);
  const char *keys[]={scope_hex,p->account,canonical,after};
  row=PQexecParams(db,"SELECT operation,result,result_sha256 FROM auth.nostr_operations_v1alpha1 WHERE namespace=$1 AND account=$2 AND identity=$3 AND action=3 AND operation COLLATE \"C\">$4 ORDER BY operation COLLATE \"C\" LIMIT 17",4,NULL,keys,NULL,NULL,0);
  if (PQresultStatus(row)!=PGRES_TUPLES_OK) {PQclear(row);goto done;}
  int count=PQntuples(row);ok=true;
  for (int i=0;i<count && i<16;i++) {
   char checksum[65]={0};coax_string sealed={PQgetvalue(row,i,1),PQgetlength(row,i,1)};
   if (!nsm_checksum(sealed,nb(checksum,64)) || strcmp(checksum,PQgetvalue(row,i,2)) || !ndl_put((coax_v_t_NdlPage){delivery_page,1},nss(PQgetvalue(row,i,0)),sealed)) {ok=false;break;}
  }
  if (count>16) {delivery_page->nn=64;memcpy(delivery_page->next.e,delivery_page->items.e[15].operation.e,64);}
  PQclear(row);if (!ok) {result.status=10;goto answer;}
  result.payload_n=ndl_page_write((coax_v_t_NdlPage){delivery_page,1},nb(payload,65536));if (result.payload_n<0) goto done;
  goto answer;
 }
 // Re-observe the canonical source under the same lock as the result commit.
 if (effect!=1) {
  auth_source=auth_identity_secret(db,p->account,canonical,secret);if (auth_source<0) goto done;
  if (strcmp(expected,canonical)) {result.status=3;goto answer;}source=nso_source(auth_source);
  bool candidate=effect==4 || (effect==3 && receipt->n>0);
  result.status=effect==4 ? nsm_commit_permit(facts,source,false):nso_cache_permit(facts,source,candidate);if (result.status) goto answer;
  int64_t action=effect==2 ? custody->action:receipt->action;
  if (effect==3 && action==3 && candidate) {result.status=5;goto answer;}memcpy(operation,effect==2 ? custody->operation.e:receipt->operation.e,64);
  if (effect==2) {
   // A managed endpoint never asks for, downloads or substitutes a linked key.
   if (source!=1) {result.status=2;goto answer;}
   if (nso_cache_action(action) && !nso_digest(cv,nb(secret,32),nb(digest,64))) goto done;
  } else memcpy(digest,receipt->digest.e,64);
  if (effect==3 || nso_cache_action(action)) {
   const char *key[]={scope_hex,operation};row=PQexecParams(db,"SELECT identity,action::text,digest,source::text,result,result_sha256 FROM auth.nostr_operations_v1alpha1 WHERE namespace=$1 AND operation=$2",2,NULL,key,NULL,NULL,0);
   if (PQresultStatus(row)!=PGRES_TUPLES_OK) {PQclear(row);goto done;}
   if (PQntuples(row)==1) {
    result.status=strcmp(PQgetvalue(row,0,0),canonical) ? 10:nso_replay(action,nss(digest),source,strtoll(PQgetvalue(row,0,1),NULL,10),nss(PQgetvalue(row,0,2)),strtoll(PQgetvalue(row,0,3),NULL,10));
    int n=PQgetlength(row,0,4);if (!result.status && (n<1 || n>65536 || !nso_candidate(action,nss(canonical),(coax_string){PQgetvalue(row,0,4),n}))) result.status=10;
    char checksum[65]={0};if (!result.status && (!nsm_checksum((coax_string){PQgetvalue(row,0,4),n},nb(checksum,64)) || strcmp(checksum,PQgetvalue(row,0,5)))) result.status=10;
    if (!result.status) {memcpy(payload,PQgetvalue(row,0,4),(size_t)n);crypto.n=n;result.payload_n=n;}PQclear(row);goto answer;
   }PQclear(row);
   if (effect==3 && !candidate) {result.status=2;goto answer;}
   const char *count_key[]={scope_hex};row=PQexecParams(db,"SELECT count(*)::text FROM auth.nostr_operations_v1alpha1 WHERE namespace=$1",1,NULL,count_key,NULL,NULL,0);
   ok=PQresultStatus(row)==PGRES_TUPLES_OK && PQntuples(row)==1;if (!ok) {PQclear(row);goto done;}bool room=nso_room(strtoll(PQgetvalue(row,0,0),NULL,10));PQclear(row);if (!room) {result.status=7;goto answer;}
  }
  if (effect==4) {
   result=nsm_stage(sv,nb(document,8388608),bv,facts,source,false,(int64_t)time(NULL),nb(payload,65536));if (result.status) goto answer;crypto.n=result.payload_n;crypto.cacheable=true;
  } else if (effect==3) {
   if (!nso_candidate(action,nss(canonical),(coax_string){(char *)receipt->result.e,receipt->n})) {result.status=5;goto answer;}
   memcpy(payload,receipt->result.e,(size_t)receipt->n);crypto.n=receipt->n;crypto.cacheable=true;
  } else {
   coax_v_t_NscPurpose pv={purpose,1};if (!nsp_read(action,nss(canonical),(coax_string){(char *)custody->payload.e,custody->n},pv)) {result.status=5;goto answer;}
   int64_t hn=0;if (action==5) {hn=nms_head(sv,nss(canonical),purpose->kind,(int64_t)time(NULL),nb(head,65536));if (hn<0) goto done;}
   if (!auth_random((unsigned char *)fresh,11*sizeof *fresh)) goto done;
   coax_t_NsFacts provider={.live=true,.human=facts.authorized,.limited=p->limited,.frozen=p->frozen,.matched=true,.approved=true,.source=source,.provider=1};
   if (action==3) {
    for (int64_t i=0;i<=purpose->peers_n;i++) {
     unsigned char pub[64];if (nsm_peer(cv,i,nb(pub,64))!=64) goto done;
     int64_t n=nms_head(sv,(coax_string){(char *)pub,64},10050,(int64_t)time(NULL),nb(head,65536));
     if (n<=0) {result.status=2;goto answer;}if (!nsm_head_put(hv,i,(coax_string){(char *)head,n})) goto done;
    }
    crypto=nsm_seal(cv,provider,nb(secret,32),(coax_v_t_Nip59Fresh){fresh,purpose->peers_n+1},hv,(int64_t)time(NULL),nb(send_body,1048576));
    if (!crypto.status) {
     if (!nsm_send_read((coax_string){(char *)send_body,crypto.n},bv)) goto done;
     result=nsm_stage(sv,nb(document,8388608),bv,facts,source,true,(int64_t)time(NULL),nb(payload,65536));
     crypto.status=result.status;crypto.n=result.payload_n;
    }
   } else crypto=nsc_materialize(cv,provider,nb(secret,32),(coax_v_t_Nip59Fresh){fresh,1},(int64_t)time(NULL),(coax_string){(char *)head,hn},nb(payload,65536));
   result.status=crypto.status;if (result.status) goto answer;
   if (crypto.cacheable && !nso_candidate(action,nss(canonical),(coax_string){(char *)payload,crypto.n})) {result.status=10;goto answer;}
  }
  if (action!=3 || effect==3) {result.length=state->j.e[0].len;result.revision=revision;}result.payload_n=crypto.n;save_receipt=crypto.cacheable;
 } else result=nms_prepare(sv,nb(document,8388608),nb(replacement,8388608),iv,facts,(int64_t)time(NULL),nb(payload,65536));
 if (result.status) goto answer;
 if (result.changed || total==0) {
  char rev[32];snprintf(rev,sizeof rev,"%lld",(long long)result.revision);
  const char *values[]={scope_hex,p->account,origin,canonical,rev,result.pruned ? "true":"false",(const char *)(result.replaced ? replacement:document)};
  int lengths[]={0,0,0,0,0,0,(int)result.length};int formats[]={0,0,0,0,0,0,1};Oid types[]={0,0,0,0,0,0,17};
  row=PQexecParams(db,"INSERT INTO auth.nostr_journals_v1alpha1(namespace,account,origin_account,identity,revision,pruned,document) VALUES($1,$2,$3,$4,$5::bigint,$6::boolean,$7) ON CONFLICT(namespace) DO UPDATE SET revision=EXCLUDED.revision,pruned=EXCLUDED.pruned,document=EXCLUDED.document,updated_at=now() WHERE auth.nostr_journals_v1alpha1.account=EXCLUDED.account RETURNING namespace",7,types,values,lengths,formats,0);
  ok=PQresultStatus(row)==PGRES_TUPLES_OK && PQntuples(row)==1;PQclear(row);if (!ok) goto done;
 }
 if (result.clear_operations) {
  const char *key[]={scope_hex};row=PQexecParams(db,"DELETE FROM auth.nostr_operations_v1alpha1 WHERE namespace=$1",1,NULL,key,NULL,NULL,0);ok=PQresultStatus(row)==PGRES_COMMAND_OK;PQclear(row);if (!ok) goto done;
  row=PQexecParams(db,"DELETE FROM auth.nostr_read_markers_v1 WHERE namespace=$1",1,NULL,key,NULL,NULL,0);ok=PQresultStatus(row)==PGRES_COMMAND_OK;PQclear(row);if (!ok) goto done;
 }
 if (save_receipt) {
  char action[24],provider[24],checksum[65]={0};if (!nsm_checksum((coax_string){(char *)payload,crypto.n},nb(checksum,64))) goto done;snprintf(action,sizeof action,"%lld",(long long)(effect==2 ? custody->action:receipt->action));snprintf(provider,sizeof provider,"%lld",(long long)source);payload[crypto.n]=0;
  const char *values[]={scope_hex,p->account,canonical,operation,action,digest,provider,(char *)payload,checksum};
  row=PQexecParams(db,"INSERT INTO auth.nostr_operations_v1alpha1(namespace,account,identity,operation,action,digest,source,result,result_sha256) VALUES($1,$2,$3,$4,$5::smallint,$6,$7::smallint,$8,$9)",9,NULL,values,NULL,NULL,0);
  ok=PQresultStatus(row)==PGRES_COMMAND_OK;PQclear(row);if (!ok) goto done;
 }
 if (!sql_ok(db,"COMMIT")) goto done;txn=false;
answer:
 if (txn) {sql_ok(db,"ROLLBACK");txn=false;}
 {
  if (effect==5 || effect==6) {
   if (result.status) {error=result.status==3 ? 403:result.status==11 ? 409:503;message=result.status==3 ? "Identity changed.":result.status==11 ? "History needs reconciliation.":"History unavailable.";goto done;}
   api_respond(q->fd,200,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",(const char *)payload,(size_t)result.payload_n);error=0;goto done;
  }
  crypto.status=result.status;crypto.n=result.payload_n;
  int64_t n=effect==1 ? nms_response(iv,result,nb(payload,65536),nb(response,400000)):effect==2 ? nsc_response(cv,crypto,nb(payload,65536),nb(response,400000)):nso_response(ov,result.status,(coax_string){(char *)payload,result.payload_n},nb(response,400000));if (n<0) goto done;
  api_respond(q->fd,200,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",(const char *)response,(size_t)n);error=0;
 }
done:
 if (txn) sql_ok(db,"ROLLBACK");
 if (payload) explicit_bzero(payload,65536);if (response) explicit_bzero(response,400000);
 explicit_bzero(secret,sizeof secret);if (custody) explicit_bzero(custody,sizeof *custody);if (purpose) explicit_bzero(purpose,sizeof *purpose);if (fresh) explicit_bzero(fresh,11*sizeof *fresh);
 free(input);free(state);free(records);free(document);free(replacement);free(arena);free(payload);free(response);free(custody);free(receipt);free(purpose);free(fresh);free(head);free(heads);free(batch);free(send_body);
 free(delivery);free(delivery_page);free(read_marker);
 if (error) api_error(q->fd,error,message);return true;
}
