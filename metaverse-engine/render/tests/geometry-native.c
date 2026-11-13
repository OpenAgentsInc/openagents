// File/ABI transport only. All geometry and draw admission stays in Coax.
#include "geometry.h"
#include <assert.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void *payload(FILE *config,int64_t *words) {
  char path[4096];assert(fscanf(config,"%4095s",path)==1);
  if(!strcmp(path,"-")){*words=0;return calloc(1,1);}
  FILE *f=fopen(path,"rb");assert(f);
  assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>=0 && n%4==0);
  assert(!fseek(f,0,SEEK_SET));void *p=calloc(n?n:1,1);assert(p);
  assert(fread(p,1,n,f)==(size_t)n);assert(!fclose(f));*words=n/4;return p;
}
int main(int argc,char **argv) {
  assert(argc==2);FILE *f=fopen(argv[1],"r");assert(f);
  int64_t profile;coax_t_FrameCounts c={0};
  assert(fscanf(f,"%" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64 " %" SCNd64,
    &profile,&c.width,&c.height,&c.hud_vertices,&c.vertex_stride,&c.draw_count,&c.water_draws,&c.grass_instances,&c.particle_instances,&c.surface_vertices,&c.caster_vertices,&c.caster_instances)==12);
  assert(c.draw_count>=0 && c.draw_count<=4096);
  int64_t n;coax_v_f32 u={0},hud={0};coax_v_u32 draws={0},indices={0};
  u.data=payload(f,&u.count);hud.data=payload(f,&hud.count);
  draws.data=payload(f,&draws.count);indices.data=payload(f,&indices.count);
  assert(fscanf(f,"%" SCNd64,&n)==1 && n>=0 && n<=48);
  coax_t_RenderGeometryBuffer *bs=calloc(n?n:1,sizeof*bs);assert(bs);
  for(int64_t i=0;i<n;i++) {
    assert(fscanf(f,"%" SCNd64 " %" SCNd64,&bs[i].binding_set,&bs[i].resource)==2);
    bs[i].words.data=payload(f,&bs[i].words.count);
  }
  assert(!fclose(f));
  coax_t_RenderDrawDomain *ds=calloc(c.draw_count?c.draw_count:1,sizeof*ds);assert(ds);
  for(int64_t i=0;i<c.draw_count;i++)ds[i].vertices=UINT64_C(99);
  coax_v_t_RenderDrawDomain domains={ds,c.draw_count};
  coax_t_RenderGeometryResult g=aethyr_geometry_probe(profile,c,draws,indices,u,(coax_v_t_RenderGeometryBuffer){bs,n},hud,domains);
  coax_t_RenderFrameResult frame=aethyr_geometry_frame_probe(c,g.direct,draws,indices,domains);
  printf("{\"geometry_status\":%u,\"frame_status\":%u,\"bad_draw\":%" PRId64 ",\"bad_resource\":%" PRId64 ",\"work\":%" PRId64 ",\"direct\":[%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 "],\"domains\":[",(unsigned)g.status,(unsigned)frame.status,g.bad_draw,g.bad_resource,g.work,g.direct.hud_vertices,g.direct.grass_instances,g.direct.particle_instances,g.direct.surface_vertices,g.direct.caster_vertices,g.direct.caster_instances);
  for(int64_t i=0;i<c.draw_count;i++) {
    if(i)putchar(',');coax_t_RenderDrawDomain d=ds[i];
    printf("[%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 "]",d.vertex_begin,d.vertices,d.vertex_alt_begin,d.vertex_alt_end,d.instance_begin,d.instances,d.indices);
  }
  printf("],\"frame_bad_upload\":%" PRId64 ",\"frame_bad_draw\":%" PRId64 ",\"frame_bad_direct\":%" PRId64 ",\"frame_checked_indices\":%" PRId64 ",\"frame_admitted_draws\":%" PRId64 "}\n",frame.bad_upload,frame.bad_draw,frame.bad_direct,frame.checked_indices,frame.admitted_draws);
  for(int64_t i=0;i<n;i++)free(bs[i].words.data);
  free(bs);free(ds);free(u.data);free(hud.data);free(draws.data);free(indices.data);return 0;
}
