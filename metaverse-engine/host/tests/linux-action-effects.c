// Firm reason: 5 (thin actual realm dispatcher and recording-face effects; Coax owns the workload and verdict).
#include "ActionCheck.h"
#include "action-fixture.h"
#include "realm-host.h"
#include <stdlib.h>
#include <string.h>
static RealmHost host;
extern void coaxial_action_fixture_mode(int32_t);
extern int64_t coaxial_action_fixture_probe(int32_t);
extern int64_t coaxial_host_probe(cx_face_addr,int32_t,int32_t);
int rhn_action_span(uint64_t address,uint64_t capacity,uint64_t bytes,uint64_t limit,int present){return hap_span(address,capacity,bytes,limit,present!=0);}
void hlac_open(int64_t mode){rh_open(&host,0);coaxial_action_fixture_mode((int32_t)mode);host.location_hash=malloc(9);if(!host.location_hash)abort();memcpy(host.location_hash,"unchanged",9);host.location_hash_bytes=9;}
void hlac_trigger(void){rh_send(&host,900,"hash",4);}
void hlac_poll(void){rh_net_poll(&host);}
void hlac_frame(void){rh_frame(&host,320,180,1,0.015625f);}
int64_t hlac_probe(int64_t field){return coaxial_action_fixture_probe((int32_t)field);}
int64_t hlac_observe(int64_t event,int64_t field){return coaxial_host_probe(host.state,(int32_t)event,(int32_t)field);}
int64_t hlac_effect(void){size_t n=0;const char *p=rh_hash(&host,&n);return n!=9||(n&&memcmp(p,"unchanged",9));}
void hlac_close(void){rh_close(&host);}
int64_t hlac_text(coax_v_u8 text){return rh_text(&host,(const char*)text.data,(size_t)text.count);}
extern int32_t guard_http(cx_face_addr,uint32_t,cx_face_addr,uint32_t,uint32_t,int64_t);
extern int32_t guard_cancel(int32_t);
int32_t coaxial_http_bounded(cx_face_addr u,uint32_t n,cx_face_addr b,uint32_t bn,int32_t flags,int64_t timeout){return guard_http(u,n,b,bn,(uint32_t)flags,timeout);}
int32_t coaxial_http_cancel(int32_t id){return guard_cancel(id);}
