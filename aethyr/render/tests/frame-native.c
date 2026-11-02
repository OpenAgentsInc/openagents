/* Native transport, with no admission decisions. Little-endian LP64 is the
 * recorded test profile. Every input record is inventoried by check_frame.py. */
#include "frame.h"
#include <assert.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

typedef struct {uint32_t first,count,base_instance,instances;} PublishedDraw;
_Static_assert(sizeof(PublishedDraw)==16,"published Draw remains four u32");
_Static_assert(offsetof(PublishedDraw,instances)==12,"published Draw field order");
_Static_assert(sizeof(coax_t_FrameCounts)==88,"count transport");
_Static_assert(sizeof(coax_t_RenderFrameLimits)==48,"limit transport");
_Static_assert(sizeof(coax_t_RenderDirectDomains)==48,"direct transport");
_Static_assert(sizeof(coax_t_RenderUpload)==56,"upload transport");
_Static_assert(sizeof(coax_t_RenderDrawDomain)==56,"domain transport");

static void read_exact(void *p,size_t item,size_t count) {assert(fread(p,item,count,stdin)==count);}
static void result(coax_t_RenderFrameResult r) {
  printf("%u %" PRId64 " %" PRId64 " %" PRId64 " %" PRId64 " %" PRId64 " ",
    (unsigned)r.status,r.bad_upload,r.bad_draw,r.bad_direct,r.checked_indices,r.admitted_draws);
}
int main(int argc,char **argv) {
  uint32_t one=1;assert(*(unsigned char *)&one==1);
  const char *fault=argc>1?argv[1]:"";
  uint32_t cases;read_exact(&cases,4,1);
  for(uint32_t i=0;i<cases;i++) {
    coax_t_FrameCounts c;coax_t_RenderFrameLimits l;coax_t_RenderDirectDomains d;
    coax_t_RenderUpload uploads[49];coax_t_RenderDrawDomain domains[65];
    uint32_t words[261],indices[257],n[4];int64_t record;
    read_exact(&c,sizeof(c),1);read_exact(&l,sizeof(l),1);read_exact(&d,sizeof(d),1);
    read_exact(n,4,4);read_exact(&record,8,1);
    assert(n[0]<=49 && n[1]<=261 && n[2]<=65 && n[3]<=257);
    read_exact(uploads,sizeof(*uploads),n[0]);read_exact(words,4,n[1]);
    read_exact(domains,sizeof(*domains),n[2]);read_exact(indices,4,n[3]);
    coax_v_t_RenderUpload u={n[0]?uploads:NULL,n[0]};
    coax_v_t_RenderDrawDomain ds={n[2]?domains:NULL,n[2]};
    coax_v_u32 ws={n[1]?words:NULL,n[1]},is={n[3]?indices:NULL,n[3]};
    coax_t_RenderFrameResult r=aethyr_frame_admit(c,l,d,u,ws,ds,is);
    coax_t_RenderFrameResult p=aethyr_frame_preflight(l,u);
    coax_t_RenderDecodedDraw q=aethyr_frame_decode(ws,record);
    if(i==0) {
      if(!strcmp(fault,"status"))r.status=COAX_RenderFrameStatus_bad_draw;
      if(!strcmp(fault,"slot"))r.bad_draw=0;
      if(!strcmp(fault,"work"))r.checked_indices++;
      if(!strcmp(fault,"publication"))r.admitted_draws++;
      if(!strcmp(fault,"preflight"))p.bad_upload=0;
      if(!strcmp(fault,"decode"))q.base_instance++;
    }
    result(r);result(p);
    printf("%u %" PRIu32 " %" PRIu32 " %" PRIu32 " %" PRIu32 "\n",
      (unsigned)q.kind,q.first,q.count,q.base_instance,q.instances);
  }
  assert(getchar()==EOF);return 0;
}
