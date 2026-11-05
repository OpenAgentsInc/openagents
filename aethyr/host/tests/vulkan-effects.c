// Firm reason: 6 (Linux native realm SDK effects; Coax writes probes,
// expectations, JSON and admission). Uses the production realm-host API.
#include "HostCheck.h"
#include "fixture.h"
#include "realm-host.h"
#include "face.h"
#include <string.h>
extern int64_t coaxial_host_probe(uint32_t,int32_t,int32_t);
extern void coaxial_host_reset(uint32_t);
static RealmHost host;
int64_t hvn_open(void) {rh_open(&host,0);return 1;}
int64_t hvn_observe(int64_t event,int64_t field) {return coaxial_host_probe(host.state,(int32_t)event,(int32_t)field);}
void hvn_reset(void) {coaxial_host_reset(host.state);}
void hvn_asset(int64_t index,coax_v_u8 bytes) {rh_asset_data(&host,(int)index,bytes.data,(size_t)bytes.count);}
void hvn_frame(float w,float h,float dpr,float dt,int64_t still) {rh_frame_input(&host,w,h,dpr,dt,(int)still);rh_after(&host,0);}
int64_t hvn_frame_at(int64_t us,int64_t pending){uint32_t n;int result=rh_frame_at(&host,320,180,1,us,pending,0,&n);if(result==1)rh_after(&host,0);return result;}
int64_t hvn_life_event(int64_t kind,int64_t us){return rh_lifecycle_event(&host,(int)kind,us,0);}
int64_t hvn_life_info(int64_t field){return rh_lifecycle_info((int)field);}
void hvn_key(int64_t key,int64_t mods,int64_t down) {rh_key(&host,(int)key,(int)mods,(int)down);}
void hvn_pointer(float x,float y,int64_t phase,int64_t kind,int64_t id) {rh_pointer(&host,x,y,(int)phase,(int)kind,(int)id);}
void hvn_wheel(float dy) {rh_wheel(&host,dy);}
void hvn_blur(void) {rh_blur(&host);}
void hvn_send(int64_t kind,coax_v_u8 bytes) {rh_send(&host,(int)kind,(const char *)bytes.data,(size_t)bytes.count);}
void hvn_close(void) {rh_net_close();rh_close(&host);}
void hvn_poll(void) {rh_net_poll(&host);}
extern int64_t guard_now_ms(void),coaxial_now_seconds(void);
int64_t hvn_clock(int64_t kind) {return kind?coaxial_now_seconds():guard_now_ms();}
int64_t hvn_net(coax_v_u8 helper,coax_v_u8 dir) {
  char hp[4096],dp[4096];if(helper.count<1||helper.count>=sizeof hp||dir.count<1||dir.count>=sizeof dp)return -1;
  memcpy(hp,helper.data,(size_t)helper.count);hp[helper.count]=0;memcpy(dp,dir.data,(size_t)dir.count);dp[dir.count]=0;
  return rh_net_open(&host,hp,"http://127.0.0.1",9170,dp,"host-contract");
}
// The fixture uses the canonical import names; legacy faces call guard_*.
int32_t guard_http(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,int64_t);
int32_t guard_cancel(int32_t);
int32_t coaxial_http_bounded(uint32_t u,uint32_t n,uint32_t b,uint32_t bn,int32_t f,int64_t t) {return guard_http(u,n,b,bn,(uint32_t)f,t);}
int32_t coaxial_http_cancel(int32_t id) {return guard_cancel(id);}
