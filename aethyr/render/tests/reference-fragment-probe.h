/* Tooling only: observe already-computed Coax fragments; modify no values. */
static coax_t_RRVertices rfc_last_vertices;
static coax_a3_u32 rfc_last_ids;
static uint32_t rfc_last_instance,rfc_last_mask;
static coax_t_RRQuad rfc_last_quad;
static int rfc_next_lane;
static int64_t rfc_last_entry,rfc_last_words;
static void rfc_fragment(coax_t_RRStored q,coax_t_RRPixel old,coax_t_RenderPassState state,coax_t_RQColor colour,float depth){
 while(rfc_next_lane<4&&!(rfc_last_mask&(1u<<rfc_next_lane)))rfc_next_lane++;
 assert(rfc_next_lane<4);int lane=rfc_next_lane++;float px=rfc_last_quad.lanes.e[lane].position.e[0],py=rfc_last_quad.lanes.e[lane].position.e[1];
 for(unsigned k=0;k<sizeof(rfc_points)/sizeof(rfc_points[0]);k++)if(px==(float)rfc_points[k][0]+.5f&&py==(float)rfc_points[k][1]+.5f){
  fprintf(stderr,"RF_FRAGMENT {\"entry\":%lld,\"x\":%lld,\"y\":%lld,\"instance\":%u,\"ids\":[%u,%u,%u],\"old_depth\":%.9g,\"depth\":%.9g,\"stored_depth\":%.9g,\"written\":%s,\"depth_rule\":%u,\"source\":[%.9g,%.9g,%.9g,%.9g],\"fragment_words\":[",(long long)rfc_last_entry,(long long)rfc_points[k][0],(long long)rfc_points[k][1],rfc_last_instance,rfc_last_ids.e[0],rfc_last_ids.e[1],rfc_last_ids.e[2],(double)old.depth,(double)depth,(double)q.pixel.depth,q.written?"true":"false",(unsigned)state.depth,(double)colour.r,(double)colour.g,(double)colour.b,(double)colour.a);
  for(int64_t w=0;w<rfc_last_words;w++)fprintf(stderr,"%s%u",w?",":"",rfc_last_quad.lanes.e[lane].words.e[w]);fputs("],\"vertices\":[",stderr);
  for(int v=0;v<3;v++){
   fprintf(stderr,"%s{\"clip\":[%.9g,%.9g,%.9g,%.9g],\"words\":[",v?",":"",(double)rfc_last_vertices.vertices.e[v].clip.e[0],(double)rfc_last_vertices.vertices.e[v].clip.e[1],(double)rfc_last_vertices.vertices.e[v].clip.e[2],(double)rfc_last_vertices.vertices.e[v].clip.e[3]);
   for(int64_t w=0;w<rfc_last_words;w++)fprintf(stderr,"%s%u",w?",":"",rfc_last_vertices.vertices.e[v].words.e[w]);fputs("]}",stderr);
  }fputs("]}\n",stderr);
 }
}
