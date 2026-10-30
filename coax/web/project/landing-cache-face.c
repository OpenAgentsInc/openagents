/* The homepage's admitted-event cache (landing-cache.coax) and its effects:
   relay frames Coax admits into it, the fold it feeds, the page's IndexedDB
   records and the build's seed. Decisions are Coax's; this moves bytes. */
#include "landing-browser.h"
#include <stdint.h>
#include <string.h>
#define EVENTS 16384
#define BYTES 12582912
#define TABLE 32768
#define IMPORT(name) __attribute__((import_module("env"),import_name(name)))
IMPORT("coaxial_storage_put") int32_t coaxial_storage_put(int32_t,int32_t,uint32_t,int32_t,int32_t,uint32_t,int32_t,uint32_t,int32_t);
IMPORT("coaxial_storage_get") int32_t coaxial_storage_get(int32_t,int32_t,uint32_t,int32_t,int32_t,uint32_t,int32_t);
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t,uint32_t,uint32_t,uint32_t,int32_t);
extern unsigned char nl_output_bytes[4194304];
extern int64_t nl_output_count;
extern coax_v_t_LrQuery home_query(void);
extern int64_t home_fold_admitted(coax_string text);
static int64_t from_[EVENTS],size_[EVENTS],project_[EVENTS],at_[EVENTS],kind_[EVENTS],dkey_[EVENTS],slot_[TABLE];
static _Bool dead_[EVENTS];
static uint8_t id_[EVENTS*32],bytes_[BYTES],proj_[8*196608];
static coax_t_HcCache cache;
static coax_v_t_HcCache c(void) {
  if(!cache.bytes.data){
    cache.from=(coax_v_s64){from_,EVENTS};cache.size=(coax_v_s64){size_,EVENTS};cache.project=(coax_v_s64){project_,EVENTS};
    cache.at=(coax_v_s64){at_,EVENTS};cache.kind=(coax_v_s64){kind_,EVENTS};cache.dkey=(coax_v_s64){dkey_,EVENTS};
    cache.dead=(coax_v_bool){dead_,EVENTS};cache.id=(coax_v_u8){id_,EVENTS*32};cache.slot=(coax_v_s64){slot_,TABLE};cache.bytes=(coax_v_u8){bytes_,BYTES};cache.proj=(coax_v_u8){proj_,sizeof proj_};
  }
  return (coax_v_t_HcCache){&cache,1};
}
static coax_v_u8 out(void) {return (coax_v_u8){nl_output_bytes,sizeof nl_output_bytes};}
static const char NS[]="home-nostr";
static int32_t key(char *buf,int64_t chunk) {
  if(chunk<0){buf[0]='h';buf[1]='e';buf[2]='a';buf[3]='d';return 4;}
  int32_t n=0;const char *p="chunk-";while(*p)buf[n++]=*p++;
  char d[12];int m=0;do{d[m++]=(char)('0'+chunk%10);chunk/=10;}while(chunk&&m<12);while(m)buf[n++]=d[--m];return n;
}
int64_t home_cache_frame(int64_t k,const unsigned char *text,int64_t n) {return lb_frame(home_query(),c(),k,(coax_string){(const char *)text,n});}
int64_t home_cache_absorb(const unsigned char *text,int64_t n,int64_t seed) {return hc_read(c(),(coax_string){(const char *)text,n},seed!=0);}
/* Project k's kept fold into the output, when it was made from exactly what k holds: its size, or 0. */
int64_t home_cache_projection(int64_t k) {coax_string t=hc_projection(c(),k);if(t.count<=0||t.count>(int64_t)sizeof nl_output_bytes)return 0;memcpy(nl_output_bytes,t.data,(size_t)t.count);nl_output_count=t.count;return t.count;}
/* The fold the output holds now (landing-fold-face.c fold_result), kept for project k. */
int64_t home_cache_keep(int64_t k) {if(nl_output_count<=0)return 0;hc_keep_projection(c(),k,(coax_string){(const char *)nl_output_bytes,nl_output_count});return 1;}
int64_t home_cache_issue_ids(int64_t k,int64_t offset) {nl_output_count=hc_issue_ids(c(),k,offset,out());return nl_output_count;}
int64_t home_cache_missing(const unsigned char *known,int64_t n,int64_t offset) {nl_output_count=hc_missing(c(),(coax_string){(const char *)known,n},offset,out());return nl_output_count;}
int64_t home_cache_synced(int64_t k) {return k>=0&&k<8?cache.synced.e[k]:0;}
/* Project k's kept events into the fold from entry i, about budget bytes: the
   next entry, -1 when all are in, -2 when the fold refused one. */
int64_t home_cache_feed(int64_t k,int64_t i,int64_t budget) {
  int64_t fed=0;
  while(fed<budget){
    i=hc_next(c(),k,i);if(i<0)return -1;
    coax_string json=hc_json(c(),i);if(!home_fold_admitted(json))return -2;
    fed+=json.count;i++;
  }
  return i;
}
int64_t home_cache_settle(int64_t k) {hc_settle(c(),k);return 1;}
int64_t home_cache_drop(int64_t k) {hc_compact(c(),k);return 1;}
int64_t home_cache_commit(int64_t keep) {hc_commit(c(),keep!=0);return 1;}
int64_t home_cache_persist_begin(void) {return hc_persist_begin(c());}
int64_t home_cache_chunks(void) {return hc_chunks(c());}
int64_t home_cache_gen(void) {return cache.gen;}
int64_t home_cache_rewritten(void) {int64_t r=cache.rewrite;cache.rewrite=0;return r;}
int64_t home_cache_head_chunks(void) {return cache.head_chunks;}
int64_t home_cache_put(int64_t chunk,int64_t chunks,int64_t token) {
  int64_t n=chunk<0?hc_head(c(),chunks,out()):hc_pack(c(),chunk,out());if(n<0)return -1;
  char k[24];int32_t kn=key(k,chunk);
  return coaxial_storage_put(1,(int32_t)token,(uint32_t)(uintptr_t)NS,sizeof NS-1,1,(uint32_t)(uintptr_t)k,kn,(uint32_t)(uintptr_t)nl_output_bytes,(int32_t)n);
}
int64_t home_cache_get(int64_t chunk,int64_t token) {
  char k[24];int32_t kn=key(k,chunk);
  return coaxial_storage_get(0,(int32_t)token,(uint32_t)(uintptr_t)NS,sizeof NS-1,1,(uint32_t)(uintptr_t)k,kn);
}
/* The build's seed part i (coax/web/project/landing.ts), named by its capture. */
int64_t home_cache_seed(int64_t captured,int64_t part) {
  char url[96];int n=0;const char *p="/build/home-seed/";while(*p)url[n++]=*p++;
  char d[24];int m=0;do{d[m++]=(char)('0'+captured%10);captured/=10;}while(captured&&m<20);while(m)url[n++]=d[--m];
  url[n++]='-';m=0;do{d[m++]=(char)('0'+part%10);part/=10;}while(part&&m<20);while(m)url[n++]=d[--m];
  p=".txt";while(*p)url[n++]=*p++;
  return coaxial_http((uint32_t)(uintptr_t)url,(uint32_t)n,0,0,4);
}
