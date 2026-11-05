// Firm reason: 5 (thin C ABI projection: calls the realm dispatcher; Coax owns the workload and verdict).
#include "ProductCheck.h"
#include "realm-host.h"
#include "audio-host.h"
#include <time.h>
#include <stdbool.h>
static RealmHost host;
static int opened;
extern void coaxial_audio_fixture_mode(int);
extern int64_t coaxial_audio_fixture_probe(int);
extern int64_t coaxial_host_probe(uint32_t,int32_t,int32_t);
extern int32_t guard_http(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,int64_t);
extern int32_t guard_cancel(int32_t);
int32_t coaxial_http_bounded(uint32_t u,uint32_t n,uint32_t b,uint32_t bn,int32_t flags,int64_t timeout){return guard_http(u,n,b,bn,(uint32_t)flags,timeout);}
int32_t coaxial_http_cancel(int32_t id){return guard_cancel(id);}
int64_t hpr_open(int64_t mode){rh_open(&host,0);opened=1;coaxial_audio_fixture_mode((int)mode);int result=rh_audio_enable(&host,1);if(host.audio)(void)rha_warm_bank(host.audio);return result;}
int64_t hpr_info(int64_t field){if(field==50)return coaxial_audio_fixture_probe(1);if(field==51)return coaxial_host_probe(host.state,-1,4);return rh_audio_info(&host,(int)field);}
void hpr_send(coax_v_u8 bytes){rh_send(&host,930,(const char*)bytes.data,(size_t)bytes.count);}
void hpr_after(void){(void)rh_after(&host,0);}
void hpr_gesture(void){rh_key(&host,'a',0,1);rh_key(&host,'a',0,0);}
void hpr_shown(bool shown){struct timespec now;clock_gettime(CLOCK_MONOTONIC,&now);(void)rh_lifecycle_event(&host,shown?5:4,(int64_t)now.tv_sec*1000000+now.tv_nsec/1000,0);}
int64_t hpr_frame(void){return rh_frame(&host,320,180,1,0.015625f);}
void hpr_retire(void){rh_audio_retire(&host);}
int64_t hpr_close(void){if(opened){rh_close(&host);opened=0;}return host.audio==NULL&&host.audio_device==NULL;}
