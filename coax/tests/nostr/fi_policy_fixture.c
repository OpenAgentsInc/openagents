/* Test-only ABI/effect-observation fixture. Product policy remains Coax. */
#include "FI_SOURCE"
#include <stdbool.h>
#include <string.h>
#define EX(name) __attribute__((export_name(name)))
static coax_string text(const char *s){return(coax_string){(char*)s,(int64_t)strlen(s)};}
static uint8_t input[16];
EX("put") void fi_put(int32_t i,int32_t value){if(i>=0&&i<16)input[i]=(uint8_t)value;}
EX("address") int32_t fi_address_probe(int32_t length,int32_t transport){return fi_address_allowed((coax_v_u8){input,length},transport);}
EX("scenario") int32_t fi_scenario(int32_t scenario){
 coax_string authorized[]={text("https://issuer.test")}, other[]={text("https://other.test")};
 coax_string audiences[]={text("wss://relay.test")},owners[]={text("human")},clients[]={text("service")};
 coax_t_FiCommunity communities[2]={
  {.host=text("relay.test"),.audience=text("wss://relay.test"),.issuers={authorized,1}},
  {.host=text("other.test"),.audience=text("wss://other.test"),.issuers={other,1}}};
 coax_t_FiIssuer p={.issuer=text("https://issuer.test"),.jwks_uri=text("https://keys.test/jwks"),.transport=1,.token_class=1,.maximum_age=300,.skew=0,.connection_lifetime=60,.refresh=30,.hard_deadline=600};
 coax_t_FiHeader header={.algorithm=text("RS256"),.typ=text("nip-fi+jwt"),.kid=text("old")};
 coax_t_FiClaims claims={.issuer=p.issuer,.subject=text("opaque-account"),.audiences={audiences,1},.key=text("0000000000000000000000000000000000000000000000000000000000000000"),.has_iat=true,.iat=100,.has_exp=true,.exp=1000};
 uint8_t modulus[256]={128},exponent[]={1,0,1};modulus[255]=1;
 coax_t_FiRsaKey keys[2]={{.kid=text("old"),.kty=text("RSA"),.algorithm=text("RS256"),.use=text("sig"),.modulus={modulus,256},.exponent={exponent,3}}};keys[1]=keys[0];keys[1].kid=text("new");
 coax_a32_u8 policy_id={0};policy_id.e[0]=42;
 coax_t_FiSnapshot snapshot={.issuer=p.issuer,.source_uri=p.jwks_uri,.policy_id=policy_id,.generation=1,.fetched_at=100,.available=true,.keys={keys,1}};
 coax_t_FiAddress answers[16]={{.length=4,.bytes={{8,8,8,8}}}};
 coax_t_FiFetch fetch={.answers={answers,1},.peer=answers[0],.resolutions=1,.connections=1,.status=200,.streamed_bytes=512,.pin_enforced=true,.tls_identity_ok=true};
 coax_string host=text("relay.test");int64_t now=100;
 switch(scenario){
 case 0:break;
 case 1:claims.issuer=text("https://unknown.test");snapshot.available=false;break;
 case 2:host=text("unknown.test");break;
 case 3:host=text("other.test");audiences[0]=text("wss://other.test");snapshot.available=false;break;
 case 4:audiences[0]=text("wss://other.test");break;
 case 5:header.algorithm=text("HS256");break;
 case 6:snapshot.available=false;break;
 case 7:header.kid=text("missing");break;
 case 8:snapshot.keys=(coax_v_t_FiRsaKey){keys+1,1};break;
 case 9:snapshot.keys.count=2;header.kid=text("new");break;
 case 10:snapshot.issuer=text("https://other.test");break;
 case 11:snapshot.policy_id.e[31]=1;break;
 case 12:now=700;claims.iat=690;break;
 case 13:snapshot.revoked=true;break;
 case 14:snapshot.keys.count=0;break;
 case 15:keys[0].has_private=true;break;
 case 16:answers[1]=answers[0];answers[1].bytes.e[0]=127;fetch.answers.count=2;break;
 case 17:fetch.peer.bytes.e[0]=9;break;
 case 18:fetch.redirected=true;break;
 case 19:fetch.streamed_bytes=524289;break;
 case 20:claims.exp=100;break;
 case 21:header.typ=text("nip-fi-agent+jwt");break;
 case 22:header.typ=text("nip-fi-agent+jwt");claims.has_owner=true;claims.owner=text("1000000000000000000000000000000000000000000000000000000000000000");break;
 case 23:p.token_class=2;p.marker_name=text("type");p.owner_values=(coax_v_string){owners,1};p.client_values=(coax_v_string){clients,1};header.typ=text("at+jwt");claims.client_id=text("phone");claims.marker=text("human");break;
 case 24:p.token_class=2;header.typ=text("at+jwt");claims.client_id=text("phone");break;
 case 25:case 26:p.token_class=2;p.marker_name=text("type");p.owner_values=(coax_v_string){owners,1};p.client_values=(coax_v_string){clients,1};header.typ=text("at+jwt");claims.client_id=text("phone");claims.marker=text("service");if(scenario==26){p.allow_client=true;p.noncollision=true;}break;
 case 27:claims.has_nbf=true;claims.nbf=101;break;
 case 28:claims.iat=0;now=300;break;
 case 29:header.typ=text("JWT");break;
 case 30:claims.has_owner=true;break;
 case 31:snapshot.last_refresh_failed=true;break;
 case 32:snapshot.source_uri=text("https://other.test");break;
 case 33:snapshot.generation=0;break;
 case 34:keys[1].kid=keys[0].kid;snapshot.keys.count=2;break;
 case 35:fetch.status=404;break;
 case 36:fetch.elapsed_ms=9999;break;
 case 37:fetch.elapsed_ms=10000;break;
 case 38:for(int i=1;i<16;i++)answers[i]=answers[0];fetch.answers.count=16;break;
 case 39:case 40:p.transport=2;p.jwks_uri=text("http://keys.local/jwks");snapshot.source_uri=p.jwks_uri;answers[0].bytes.e[0]=10;fetch.peer=answers[0];fetch.tls_identity_ok=false;fetch.mesh_endpoint_authorized=scenario==40;break;
 case 41:p.jwks_uri=text("http://keys.test");break;
 case 42:fetch.peer.length=16;break;
 default:return 99;
 }
 /* Fetch evidence is validated at refresh, not fetched during admission. */
 if(!fi_fetch_candidate(p.jwks_uri,p.transport,fetch))snapshot.available=false;
 coax_t_FiKeySelection candidate=fi_assertion_candidate((coax_v_t_FiCommunity){communities,2},(coax_v_t_FiIssuer){&p,1},host,header,claims,now,snapshot,policy_id,1);
 return(int32_t)candidate.status;
}
