// Platform effects: authenticated SQL, existing sealed-key service and response I/O.
// Account admission, field validation and every JSON byte are shared Coax.
#include "auth-internal.h"
#include "account-settings-wire.generated.h"
#include "auth-recover-crypto.generated.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
static coax_string css(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 cb(char *s,size_t n) {return (coax_v_u8){(uint8_t *)s,(int64_t)n};}
static void answer(const request *q,const char *body,int64_t n) {
 if (n<0) {api_error(q->fd,503,"Settings unavailable.");return;}
 api_respond(q->fd,200,"application/json","Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n",body,(size_t)n);
}
static bool load(PGconn *db,const char *account,coax_t_AccountSettings *s,char *raw,size_t cap) {
 const char *v[]={account};PGresult *r=PQexecParams(db,"SELECT document FROM auth.account_settings_v1 WHERE account=$1",1,NULL,v,NULL,NULL,0);
 bool ok=PQresultStatus(r)==PGRES_TUPLES_OK;
 int64_t n=-1;if (ok && PQntuples(r)) {n=(int64_t)strlen(PQgetvalue(r,0,0));if ((size_t)n>=cap) ok=false;else memcpy(raw,PQgetvalue(r,0,0),(size_t)n+1);}
 else if (ok) {n=asw_initial(cb(raw,cap-1));if (n>=0) raw[n]=0;else ok=false;}
 PQclear(r);return ok && asw_read((coax_string){raw,n},(coax_v_t_AccountSettings){s,1});
}
bool account_settings_route(const request *q,PGconn *db,const char *op) {
 bool settings=!strcmp(op,"settings/v1"),backup=!strcmp(op,"identity/backup/v1"),profile=!strcmp(op,"settings/profile-event/v1");
 if (!settings && !backup && !profile) return false;
 api_route_at("account_settings_route");
 const auth_principal *p=auth_principal_of(q);
 if (!p || !p->account[0] || !p->session[0]) {api_error(q->fd,401,"Log in first.");return true;}
 if (!db) {api_error(q->fd,503,"Settings unavailable.");return true;}
 bool get=!strcmp(q->method,"GET"),post=!strcmp(q->method,"POST");
 if ((!get && !post) || (get && !settings)) {api_error(q->fd,405,"Use the settings operation's method.");return true;}
 if (post && !asp_write(p->kind,true,p->limited,p->frozen)) {api_error(q->fd,403,"Verify your account before changing it.");return true;}
 if (backup && !asp_backup(p->kind,true,p->limited,p->frozen,p->level,p->l2_age_ms)) {auth_step_up_refusal(q,db,p,3,401,"Verify with a passkey or login key first.");return true;}
 char raw[8193];coax_t_AccountSettings s={0};coax_v_t_AccountSettings view={&s,1};
 if (backup || profile) {
  unsigned char secret[32]={0};char pubkey[65]={0},plain[65]={0};
  int source=auth_identity_secret(db,p->account,pubkey,secret);
  if (source!=2) {explicit_bzero(secret,sizeof secret);api_error(q->fd,source==1 ? 409:503,source==1 ? "This identity's private key is held outside OpenAgents.":"Identity unavailable.");return true;}
  if (backup) {
   auth_hex(secret,32,plain);int64_t n=asw_backup(css(pubkey),css(plain),cb(raw,sizeof raw-1));
   auth_audit(db,p->account,p->session,"identity.backup","ok","{}");answer(q,raw,n);explicit_bzero(raw,sizeof raw);explicit_bzero(plain,sizeof plain);
  } else {
   char content[8192],event[16384];unsigned char aux[32];
   int64_t n=load(db,p->account,&s,raw,sizeof raw) ? asw_profile(view,cb(content,sizeof content)):-1;
   int64_t k=n>0 && auth_random(aux,sizeof aux) ? arc_profile_event((coax_v_u8){secret,32},(coax_v_u8){aux,32},(int64_t)time(NULL),(coax_string){content,n},cb(event,sizeof event)):-1;
   answer(q,event,k);explicit_bzero(aux,sizeof aux);
  }
  explicit_bzero(secret,sizeof secret);return true;
 }
 if (get) {if (!load(db,p->account,&s,raw,sizeof raw)) api_error(q->fd,503,"Settings unavailable.");else answer(q,raw,(int64_t)strlen(raw));return true;}
 bool ok=q->body_len<=8192 && asw_read((coax_string){q->body,(int64_t)q->body_len},view);
 if (!ok) {api_error(q->fd,400,"Check the profile and relay values.");return true;}
 char expected[32],revision[32];snprintf(expected,sizeof expected,"%lld",(long long)s.revision);s.revision++;
 int64_t n=asw_write(view,cb(raw,sizeof raw-1));if (n<0) {api_error(q->fd,400,"Settings too large.");return true;}raw[n]=0;snprintf(revision,sizeof revision,"%lld",(long long)s.revision);
 const char *v[]={p->account,revision,raw,expected};
 PGresult *r=PQexecParams(db,"INSERT INTO auth.account_settings_v1(account,revision,document) SELECT $1,$2::bigint,$3 WHERE $4::bigint=0 ON CONFLICT(account) DO UPDATE SET revision=$2::bigint,document=$3 WHERE auth.account_settings_v1.revision=$4::bigint RETURNING revision",4,NULL,v,NULL,NULL,0);
 // A nonzero expected revision uses UPDATE, since INSERT SELECT has no row.
 if (PQresultStatus(r)==PGRES_TUPLES_OK && !PQntuples(r) && strcmp(expected,"0")) {
  PQclear(r);r=PQexecParams(db,"UPDATE auth.account_settings_v1 SET revision=$2::bigint,document=$3 WHERE account=$1 AND revision=$4::bigint RETURNING revision",4,NULL,v,NULL,NULL,0);
 }
 ok=PQresultStatus(r)==PGRES_TUPLES_OK;bool changed=ok && PQntuples(r)==1;PQclear(r);
 if (!ok) api_error(q->fd,503,"Settings unavailable.");else if (!changed) api_error(q->fd,409,"Settings changed. Reload before saving.");else answer(q,raw,n);
 return true;
}
