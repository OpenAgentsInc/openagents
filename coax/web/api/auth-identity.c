// Platform effects only: account lock, OS entropy, the existing Coax crypto/seal
// services, parameterized storage. Provisioning/admission is auth-identity-policy.coax.
#include "auth-internal.h"
#include "auth-identity-policy.generated.h"
#include "auth-identity-wire.generated.h"
#include "auth-recover-crypto.generated.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_string ais(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static bool command(PGconn *db,const char *sql) {PGresult *r=PQexec(db,sql);bool ok=PQresultStatus(r)==PGRES_COMMAND_OK;PQclear(r);return ok;}

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
 r=PQexecParams(db,"SELECT pubkey,sealed,created_ms::text FROM auth.account_identity_keys_v1 WHERE account=$1 ORDER BY created_ms,pubkey LIMIT 1",1,NULL,v,NULL,NULL,0);
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

bool auth_identity_route(const request *q,PGconn *db,const char *op) {
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
