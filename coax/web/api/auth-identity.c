// Platform effects only: account lock, OS entropy, the existing Coax crypto/seal
// services, parameterized storage. Provisioning/admission is auth-identity-policy.coax.
#include "auth-internal.h"
#include "auth-identity-policy.generated.h"
#include "auth-identity-wire.generated.h"
#include "account-settings-wire.generated.h"
#include "auth-recover-crypto.generated.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_string ais(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static bool command(PGconn *db,const char *sql) {PGresult *r=PQexec(db,sql);bool ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);return ok;}
// Called under the account lock. A broken or foreign selection never falls back to another key.
static bool identity_revision(PGconn *db,const char *account,int64_t *revision) {
 const char *v[]={account};*revision=0;
 PGresult *r=PQexecParams(db,"SELECT s.pubkey,s.revision::text,k.account FROM auth.account_identity_selection_v1 s LEFT JOIN auth.account_identity_keys_v1 k ON k.pubkey=s.pubkey WHERE s.account=$1",1,NULL,v,NULL,NULL,0);
 bool ok=PQresultStatus(r)==PGRES_TUPLES_OK;
 if (ok && PQntuples(r)==1) {*revision=strtoll(PQgetvalue(r,0,1),NULL,10);ok=aicp_key(aic_selection(ais(account),ais(PQgetvalue(r,0,0)),*revision),PQgetisnull(r,0,2) || strcmp(PQgetvalue(r,0,2),account)!=0);}
 PQclear(r);return ok;
}

// -1 unavailable, 0 inactive account, 1 linked identity, 2 managed identity.
// Use a savepoint when called by an effect already inside a transaction.
int auth_identity_ensure(PGconn *db,const char *account,char pubkey[65]) {
 pubkey[0]=0;pubkey[64]=0; // arc_public writes exactly 64 hex bytes, without a C terminator.
 bool own=PQtransactionStatus(db)==PQTRANS_IDLE;
 if (!command(db,own ? "BEGIN":"SAVEPOINT account_identity_v1")) return -1;
 unsigned char secret[32]={0};char plain[65]={0},derived[65]={0},sealed[211]={0},ad[160]={0};
 const char *v[]={account};int result=-1;
 PGresult *r=PQexecParams(db,"SELECT deleted_at IS NULL AND merged_into IS NULL,frozen_at IS NOT NULL FROM auth.accounts WHERE id=$1 FOR UPDATE",1,NULL,v,NULL,NULL,0);
 if (PQresultStatus(r)!=PGRES_TUPLES_OK) {PQclear(r);goto done;}
 bool live=PQntuples(r)==1 && !strcmp(PQgetvalue(r,0,0),"t"),frozen=PQntuples(r)==1 && !strcmp(PQgetvalue(r,0,1),"t");PQclear(r);
 // An inactive account must not open a secret either.
 if (ai_plan(live,frozen,false,false)==0) {result=0;goto done;}
 int64_t revision;if (!identity_revision(db,account,&revision)) goto done;
 r=PQexecParams(db,"SELECT k.pubkey,k.sealed,k.created_ms::text FROM auth.account_identity_keys_v1 k LEFT JOIN auth.account_identity_selection_v1 s ON s.account=k.account AND s.pubkey=k.pubkey WHERE k.account=$1 ORDER BY (s.pubkey IS NOT NULL) DESC,k.created_ms,k.pubkey LIMIT 1",1,NULL,v,NULL,NULL,0);
 if (PQresultStatus(r)!=PGRES_TUPLES_OK) {PQclear(r);goto done;}
 bool managed=PQntuples(r)==1;
 if (managed) {
  if (!aiw_record(ais(account),ais(PQgetvalue(r,0,0)),ais(PQgetvalue(r,0,1)),strtoll(PQgetvalue(r,0,2),NULL,10))) {PQclear(r);goto done;}
  snprintf(pubkey,65,"%s",PQgetvalue(r,0,0));snprintf(sealed,sizeof sealed,"%s",PQgetvalue(r,0,1));
 }
 PQclear(r);
 if (!managed) {
  r=PQexecParams(db,"SELECT pubkey FROM auth.nostr_keys WHERE account=$1 ORDER BY id LIMIT 1",1,NULL,v,NULL,NULL,0);
  if (PQresultStatus(r)!=PGRES_TUPLES_OK) {PQclear(r);goto done;}
  bool linked=PQntuples(r)==1;
  int plan=ai_plan(live,frozen,managed,linked);
  if (plan==1) {snprintf(pubkey,65,"%s",PQgetvalue(r,0,0));PQclear(r);result=1;goto done;}
  PQclear(r);
  if (plan!=3 || !auth_random(secret,sizeof secret) || !arc_public((coax_v_u8){secret,32},(coax_v_u8){(uint8_t *)pubkey,64})) goto done;
  auth_hex(secret,32,plain);
 }
 int64_t n=aiw_ad(ais(pubkey),(coax_v_u8){(uint8_t *)ad,sizeof ad-1});if (n<0) goto done;ad[n]=0;
 if (!managed) {
  if (!auth_seal(ad,plain,sealed,sizeof sealed)) goto done;
  const char *insert[]={account,pubkey,sealed};
  r=PQexecParams(db,"INSERT INTO auth.account_identity_keys_v1(account,pubkey,sealed) VALUES($1,$2,$3)",3,NULL,insert,NULL,NULL,0);
  bool ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);if (!ok) goto done;
 } else {
  // Fail closed on lost/wrong sealing keys, swapped rows or a corrupt scalar.
  if (!auth_unseal(ad,sealed,plain,sizeof plain) || !auth_unhex(plain,secret,32)
   || !arc_public((coax_v_u8){secret,32},(coax_v_u8){(uint8_t *)derived,64}) || strcmp(derived,pubkey)) goto done;
 }
 result=2;
done:
 explicit_bzero(secret,sizeof secret);explicit_bzero(plain,sizeof plain);
 if (result<0) {command(db,own ? "ROLLBACK":"ROLLBACK TO SAVEPOINT account_identity_v1");pubkey[0]=0;}
 if (!own && !command(db,"RELEASE SAVEPOINT account_identity_v1")) {result=-1;pubkey[0]=0;}
 if (own && result>=0 && !command(db,"COMMIT")) {command(db,"ROLLBACK");result=-1;pubkey[0]=0;}
 return result;
}

// Opens only the existing canonical managed identity, never a login/wallet key.
// Caller admission is account-settings-policy.coax; every scratch copy is wiped.
int auth_identity_secret(PGconn *db,const char *account,char pubkey[65],unsigned char secret[32]) {
 memset(secret,0,32);int source=auth_identity_ensure(db,account,pubkey);if (source!=2) return source;
 const char *v[]={account,pubkey};PGresult *r=PQexecParams(db,"SELECT sealed FROM auth.account_identity_keys_v1 WHERE account=$1 AND pubkey=$2",2,NULL,v,NULL,NULL,0);
 char plain[65]={0},ad[160]={0},derived[65]={0};bool ok=PQresultStatus(r)==PGRES_TUPLES_OK && PQntuples(r)==1;
 int64_t n=aiw_ad(ais(pubkey),(coax_v_u8){(uint8_t *)ad,sizeof ad-1});if (n>=0) ad[n]=0;
 ok=ok && n>0 && auth_unseal(ad,PQgetvalue(r,0,0),plain,sizeof plain) && auth_unhex(plain,secret,32) && arc_public((coax_v_u8){secret,32},(coax_v_u8){(uint8_t *)derived,64}) && !strcmp(pubkey,derived);
 PQclear(r);explicit_bzero(plain,sizeof plain);if (!ok) {explicit_bzero(secret,32);return -1;}return 2;
}

// Transactional SDK carrier for the checked identity-change codec/policy.
static void identity_change(const request *q,PGconn *db,const auth_principal *p) {
 if (strcmp(q->method,"POST")) {api_error(q->fd,405,"POST this.");return;}
 if (!aicp_allowed(p->kind,p->session[0]!=0,p->limited,p->frozen,p->level,p->l2_age_ms)) {auth_step_up_refusal(q,db,p,3,401,"Verify with a passkey first.");return;}
 coax_t_IdentityChange change={0};
 if (!aic_read((coax_string){q->body,(int64_t)q->body_len},(coax_v_t_IdentityChange){&change,1})) {explicit_bzero(&change,sizeof change);api_error(q->fd,400,"Invalid identity change.");return;}
 char command_id[33]={0},expected[65]={0},digest[65]={0},pub[65]={0},current[65]={0},plain[65]={0},sealed[211]={0},ad[160]={0},body[512]={0};
 unsigned char secret[32]={0};int status=503;const char *words="Identity unavailable.";bool committed=false;
 memcpy(command_id,change.command.e,32);memcpy(expected,change.expected.e,64);auth_sha_hex(q->body,digest);
 if (!command(db,"BEGIN")) goto done;
 const char *v[]={p->account};
 PGresult *r=PQexecParams(db,"SELECT id FROM auth.accounts WHERE id=$1 AND deleted_at IS NULL AND merged_into IS NULL AND frozen_at IS NULL FOR UPDATE",1,NULL,v,NULL,NULL,0);
 bool live=PQresultStatus(r)==PGRES_TUPLES_OK && PQntuples(r)==1;PQclear(r);
 if (!live) {status=403;words="Account unavailable.";goto rollback;}
 const char *lookup[]={p->account,command_id};
 r=PQexecParams(db,"SELECT digest,pubkey,cancelled FROM auth.account_identity_changes_v1 WHERE account=$1 AND command=$2",2,NULL,lookup,NULL,NULL,0);
 if (PQresultStatus(r)!=PGRES_TUPLES_OK) {PQclear(r);goto rollback;}
 bool exists=PQntuples(r)==1,same=exists && !strcmp(PQgetvalue(r,0,2),"f") && !strcmp(PQgetvalue(r,0,0),digest);
 if (same) snprintf(pub,sizeof pub,"%s",PQgetvalue(r,0,1));PQclear(r);
 // Replay before CAS: the original key can already have been replaced again.
 int plan=aicp_plan(exists,same,false);
 if (plan==1) goto finish;
 if (exists) {status=409;words="This operation already has a different request.";goto rollback;}
 if (auth_identity_ensure(db,p->account,current)<1) goto rollback;
 int64_t revision;if (!identity_revision(db,p->account,&revision)) goto rollback;
 if (aicp_plan(false,false,!strcmp(expected,current) && aicp_revision(change.revision,revision))!=2) {status=409;words="Your identity changed. Open Settings again.";goto rollback;}
 if (change.mode==2) {memcpy(plain,change.secret.e,64);if (!auth_unhex(plain,secret,32)) {status=400;words="Invalid private key.";goto rollback;}}
 else if (!auth_random(secret,32)) goto rollback;
 bool valid=arc_public((coax_v_u8){secret,32},(coax_v_u8){(uint8_t *)pub,64});
 if (!valid) {status=400;words="Invalid private key.";goto rollback;}
 const char *key[]={pub,p->account};
 r=PQexecParams(db,"SELECT EXISTS(SELECT 1 FROM auth.account_identity_keys_v1 WHERE pubkey=$1 AND account<>$2) OR EXISTS(SELECT 1 FROM auth.nostr_keys WHERE pubkey=$1 AND account<>$2)",2,NULL,key,NULL,NULL,0);
 bool read_ok=PQresultStatus(r)==PGRES_TUPLES_OK && PQntuples(r)==1;
 bool other=read_ok && !strcmp(PQgetvalue(r,0,0),"t");PQclear(r);
 if (!read_ok) goto rollback;
 if (!aicp_key(valid,other)) {status=409;words="That key belongs to another account.";goto rollback;}
 auth_hex(secret,32,plain);int64_t n=aiw_ad(ais(pub),(coax_v_u8){(uint8_t *)ad,sizeof ad-1});if (n<0) goto rollback;ad[n]=0;
 if (!auth_seal(ad,plain,sealed,sizeof sealed)) goto rollback;
 const char *insert[]={p->account,pub,sealed};
 r=PQexecParams(db,"INSERT INTO auth.account_identity_keys_v1(account,pubkey,sealed) VALUES($1,$2,$3) ON CONFLICT(pubkey) DO UPDATE SET sealed=EXCLUDED.sealed WHERE auth.account_identity_keys_v1.account=EXCLUDED.account",3,NULL,insert,NULL,NULL,0);
 bool ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);if (!ok) goto rollback;
 // A conflicting concurrent import by another account cannot become our selection.
 const char *select[]={p->account,pub};
 r=PQexecParams(db,"INSERT INTO auth.account_identity_selection_v1(account,pubkey,revision) SELECT $1,pubkey,1 FROM auth.account_identity_keys_v1 WHERE account=$1 AND pubkey=$2 ON CONFLICT(account) DO UPDATE SET pubkey=EXCLUDED.pubkey,revision=auth.account_identity_selection_v1.revision+1 RETURNING pubkey",2,NULL,select,NULL,NULL,0);
 ok=PQresultStatus(r)==PGRES_TUPLES_OK && PQntuples(r)==1;PQclear(r);if (!ok) {status=409;words="That key belongs to another account.";goto rollback;}
 const char *journal[]={p->account,command_id,digest,expected,pub,change.mode==1 ? "rotate":"restore"};
 if (!aic_receipt(ais(journal[0]),ais(journal[1]),ais(journal[2]),ais(journal[3]),ais(journal[4]),ais(journal[5]),false)) goto rollback;
 r=PQexecParams(db,"INSERT INTO auth.account_identity_changes_v1(account,command,digest,expected,pubkey,mode) VALUES($1,$2,$3,$4,$5,$6)",6,NULL,journal,NULL,NULL,0);
 ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);if (!ok) goto rollback;
finish:
 if (!command(db,"COMMIT")) goto rollback;
 committed=true;status=200;
 auth_audit(db,p->account,p->session,"identity.change","ok","{}");
 goto done;
rollback:
 command(db,"ROLLBACK");
done:
 if (committed) {int64_t n=aiw_write(ais(p->account),ais(pub),2,(coax_v_u8){(uint8_t *)body,sizeof body});if (n>0) api_respond(q->fd,200,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",body,(size_t)n);else api_error(q->fd,503,"Identity unavailable.");}
 else api_error(q->fd,status,words);
 explicit_bzero(&change,sizeof change);explicit_bzero(secret,sizeof secret);explicit_bzero(plain,sizeof plain);explicit_bzero(sealed,sizeof sealed);
}

// Account lock serializes terminal discard with every possible delayed request.
static void identity_cancel(const request *q,PGconn *db,const auth_principal *p) {
 if (strcmp(q->method,"POST")) {api_error(q->fd,405,"POST this.");return;}
 if (!aicp_allowed(p->kind,p->session[0]!=0,p->limited,p->frozen,p->level,p->l2_age_ms)) {auth_step_up_refusal(q,db,p,3,401,"Verify with a passkey first.");return;}
 char id[33]={0},pub[65]={0},body[512];
 if (!aic_cancel_read((coax_string){q->body,(int64_t)q->body_len},(coax_v_u8){(uint8_t *)id,32})) {api_error(q->fd,400,"Invalid discard.");return;}
 int source=-1;bool ok=false;
 if (!command(db,"BEGIN")) goto done;
 // ensure takes and retains the account lock and rejects frozen/deleted accounts.
 source=auth_identity_ensure(db,p->account,pub);if (source<1) goto rollback;
 if (!aic_receipt(ais(p->account),ais(id),ais("0000000000000000000000000000000000000000000000000000000000000000"),ais(pub),ais(""),ais("cancel"),true)) goto rollback;
 const char *v[]={p->account,id,pub};
 PGresult *r=PQexecParams(db,"INSERT INTO auth.account_identity_changes_v1(account,command,digest,expected,pubkey,mode,cancelled) VALUES($1,$2,repeat('0',64),$3,NULL,'cancel',true) ON CONFLICT(account,command) DO NOTHING",3,NULL,v,NULL,NULL,0);
 ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);if (!ok) goto rollback;
 if (!command(db,"COMMIT")) goto rollback;
 auth_audit(db,p->account,p->session,"identity.discard","ok","{}");goto done;
rollback:
 command(db,"ROLLBACK");ok=false;
done:
 if (ok) {int64_t n=aiw_write(ais(p->account),ais(pub),source,(coax_v_u8){(uint8_t *)body,sizeof body});if (n>0) {api_respond(q->fd,200,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",body,(size_t)n);return;}}
 api_error(q->fd,source==0 ? 403:503,"Identity unavailable.");
}

bool auth_identity_route(const request *q,PGconn *db,const char *op) {
 if (!strcmp(op,"identity/cancel/v1")) {
  api_route_at("auth_identity_route");const auth_principal *p=auth_principal_of(q);
  if (!p || !p->account[0]) api_error(q->fd,401,"Log in first.");else if (!db) api_error(q->fd,503,"Identity unavailable.");else identity_cancel(q,db,p);return true;
 }
 if (!strcmp(op,"identity/state/v1")) {
  api_route_at("auth_identity_route");const auth_principal *p=auth_principal_of(q);
  if (strcmp(q->method,"GET")) {api_error(q->fd,405,"GET this.");return true;}
  if (!p || !p->account[0]) {api_error(q->fd,401,"Log in first.");return true;}
  char pub[65],body[512];int64_t revision=0,n=-1;int source=-1;
  if (db && command(db,"BEGIN")) {source=auth_identity_ensure(db,p->account,pub);if (source>0 && identity_revision(db,p->account,&revision)) n=aic_state_write(ais(p->account),ais(pub),source,revision,(coax_v_u8){(uint8_t *)body,sizeof body});if (n<0 || !command(db,"COMMIT")) {command(db,"ROLLBACK");n=-1;}}
  if (n<0) api_error(q->fd,source==0 ? 403:503,"Identity unavailable.");else api_respond(q->fd,200,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",body,(size_t)n);return true;
 }
 if (!strcmp(op,"identity/change/v1")) {
  api_route_at("auth_identity_route");const auth_principal *p=auth_principal_of(q);
  if (!p || !p->account[0]) api_error(q->fd,401,"Log in first.");else if (!db) api_error(q->fd,503,"Identity unavailable.");else identity_change(q,db,p);return true;
 }
 if (strcmp(op,"identity/v1")) return false;
 api_route_at("auth_identity_route"); // New v1 contract, separate from the frozen auth v1alpha1 answers.
 if (strcmp(q->method,"GET")) {api_error(q->fd,405,"GET this.");return true;}
 const auth_principal *p=auth_principal_of(q);
 if (!p || !p->account[0]) {api_error(q->fd,401,"Log in first.");return true;}
 if (!db) {api_error(q->fd,503,"Identity unavailable.");return true;}
 char pubkey[65],body[512];int source=auth_identity_ensure(db,p->account,pubkey);
 int64_t n=source>0 ? aiw_write(ais(p->account),ais(pubkey),source,(coax_v_u8){(uint8_t *)body,sizeof body}):-1;
 if (n<0) {api_error(q->fd,source==0 ? 403:503,"Identity unavailable.");return true;}
 api_respond(q->fd,200,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",body,(size_t)n);return true;
}
