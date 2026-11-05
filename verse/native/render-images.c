// SDK allocation/read ownership. No codec, URL routing or media formatting.
#include "render-images.h"
#include <stdlib.h>
#include <string.h>
#include <limits.h>
struct RhgiImages {RhgiImage *rows;size_t count,capacity;coax_t_HostImageMemory memory;};
static int rhgi_same(coax_string a,coax_string b){return a.count==b.count&&(a.count==0||memcmp(a.data,b.data,(size_t)a.count)==0);}
RhgiImages *rhgi_open(void){
  int64_t capacity=hic_capacity();if(capacity<1||(uint64_t)capacity>SIZE_MAX/sizeof(RhgiImage))return NULL;
  RhgiImages *c=calloc(1,sizeof*c);if(!c)return NULL;c->capacity=(size_t)capacity;c->rows=calloc(c->capacity,sizeof*c->rows);if(!c->rows){free(c);return NULL;}return c;
}
RhgiImage rhgi_get(RhgiImages *c,int64_t kind,coax_string url,RhgiSource source){
  RhgiImage refused={.url=url,.slot=-1,.failed=kind==1};
  if(!c||!url.data||url.count<0)return refused;
  for(size_t i=0;i<c->count;i++)if(rhgi_same(c->rows[i].url,url))return kind==1?c->rows[i]:refused;
  if(!hic_offer(kind,url,(int64_t)c->count))return refused;
  char *name=malloc((size_t)url.count+1);if(!name)return refused;memcpy(name,url.data,(size_t)url.count);name[url.count]=0;
  RhgiImage *r=&c->rows[c->count];*r=(RhgiImage){.url={name,url.count},.slot=(int64_t)c->count,.failed=1};c->count++;
  int64_t bytes=source.size?source.size(source.user,r->url):-1;if(!hic_source(bytes)||!source.read)return *r;
  uint8_t *encoded=malloc((size_t)bytes);if(!encoded)return *r;
  if(source.read(source.user,r->url,encoded,(size_t)bytes)!=1){free(encoded);return *r;}
  coax_t_HostMediaPlan p=htm_plan((coax_v_u8){encoded,bytes});
  if(p.status!=0||!hic_reserve((coax_v_t_HostImageMemory){&c->memory,1},r->slot,p.bytes)){free(encoded);return *r;}
  uint8_t *work=malloc((size_t)p.work),*pixels=malloc((size_t)p.bytes);
  if(!work||!pixels){free(work);free(pixels);free(encoded);hic_release((coax_v_t_HostImageMemory){&c->memory,1},r->slot);return *r;}
  int64_t status=htm_decode((coax_v_u8){encoded,bytes},(coax_v_u8){work,p.work},(coax_v_u8){pixels,p.bytes});free(work);free(encoded);
  if(status!=0){free(pixels);hic_release((coax_v_t_HostImageMemory){&c->memory,1},r->slot);return *r;}
  r->width=p.width;r->height=p.height;r->pixels=pixels;r->bytes=(size_t)p.bytes;r->decoded=1;r->failed=0;return *r;
}
size_t rhgi_count(const RhgiImages *c){return c?c->count:0;}
size_t rhgi_bytes(const RhgiImages *c){return c?(size_t)hic_total((coax_v_t_HostImageMemory){(coax_t_HostImageMemory*)&c->memory,1}):0;}
int64_t rhgi_notice(const coax_t_HostMediaNotice *rows,size_t count,uint8_t *out,size_t bytes){
  if(count>INT64_MAX||bytes>INT64_MAX||(!rows&&count)||(!out&&bytes))return -1;
  return htn_format((coax_v_t_HostMediaNotice){(coax_t_HostMediaNotice*)rows,(int64_t)count},(coax_v_u8){out,(int64_t)bytes});
}
int64_t rhgi_close(RhgiImages *c){if(!c)return 0;for(size_t i=0;i<c->count;i++){free((void*)c->rows[i].url.data);free((void*)c->rows[i].pixels);if(c->rows[i].decoded)hic_release((coax_v_t_HostImageMemory){&c->memory,1},(int64_t)i);}int64_t remaining=hic_total((coax_v_t_HostImageMemory){&c->memory,1});free(c->rows);free(c);return remaining;}
