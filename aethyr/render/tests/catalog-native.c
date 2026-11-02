#include "catalog.h"
#include <assert.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

static void string(coax_string s) {
  assert(s.count >= 0 && s.count < 1048576);
  putchar('"');
  for (int64_t i=0;i<s.count;i++) {
    unsigned char c=(unsigned char)s.data[i];
    if (c=='"' || c=='\\') {putchar('\\');putchar(c);}
    else if (c<32) printf("\\u%04x",c);
    else putchar(c);
  }
  putchar('"');
}
static void string_field(const char *name,coax_string s) {printf(",\"%s\":",name);string(s);}
static void features(coax_t_RenderFeatures f) {
#define FEATURE(name) printf("\"" #name "\":%s,",f.name?"true":"false")
  putchar('{');FEATURE(scene);FEATURE(shadow);FEATURE(casters);FEATURE(grass);
  FEATURE(surface);FEATURE(water);FEATURE(water_copy);FEATURE(post);FEATURE(post_depth);
  FEATURE(traced_depth);FEATURE(particles);FEATURE(particle_depth);FEATURE(particle_over);
  printf("\"media\":%s}",f.media?"true":"false");
#undef FEATURE
}
int main(int argc,char **argv) {
  const char *fault=argc>1?argv[1]:"";
  assert(!aethyr_catalog_profile(-1).valid && !aethyr_catalog_profile(18).valid);
  assert(aethyr_catalog_binding_count(0,2)==-1 && aethyr_catalog_binding(0,0,-1).kind==7);
  assert(!aethyr_catalog_entry(0,13).valid && aethyr_catalog_use_count(0,13)==-1 && aethyr_catalog_use(0,13,0).slot==-1);
  putchar('[');
  for (int64_t i=0;i<18;i++) {
    coax_t_RenderCatalog c=aethyr_catalog_profile(i);
    if (i)putchar(',');
    if (!strcmp(fault,"asset") && i==0)c.assets.count--;
    if (!strcmp(fault,"feature") && i==2)c.features.media=0;
    printf("{\"valid\":%s,\"listed\":%s,\"retired\":%s",c.valid?"true":"false",c.listed?"true":"false",c.retired?"true":"false");
    string_field("slug",c.slug);string_field("app",c.app);string_field("assets",c.assets);
#define INTEGER(name) printf(",\"" #name "\":%" PRId64,c.name)
    INTEGER(page);INTEGER(uniform_vec4s);INTEGER(vertex_stride);INTEGER(binding_first);
    INTEGER(binding_count);INTEGER(pass_first);INTEGER(pass_count);INTEGER(particle_binding_first);
    INTEGER(particle_binding_count);INTEGER(particle_pass_first);INTEGER(particle_pass_count);
#undef INTEGER
    printf(",\"features\":");features(c.features);
    printf(",\"bindings\":[");
    for (int64_t set=0;set<2;set++) {
      if(set)putchar(',');putchar('[');
      int64_t n=aethyr_catalog_binding_count(i,set);
      for(int64_t j=0;j<n;j++) {
        coax_t_RenderBinding r=aethyr_catalog_binding(i,set,j);
        if (!strcmp(fault,"binding") && i==0 && set==0 && j==1)r.binding++;
        if(j)putchar(',');printf("{\"name\":");string(r.name);
        printf(",\"kind\":%u,\"group\":%" PRId64 ",\"binding\":%" PRId64 ",\"stride\":%" PRId64 ",\"uniform_vec4s\":%" PRId64 "}",(unsigned)r.kind,r.group,r.binding,r.stride,r.uniform_vec4s);
      }
      assert(aethyr_catalog_binding(i,set,n).kind==7);
      putchar(']');
    }
    printf("],\"pipelines\":[");
    for (int p=0;p<13;p++) {
      if(p)putchar(',');coax_t_RenderEntry e=aethyr_catalog_entry(i,p);
      if(!strcmp(fault,"pipeline") && i==0 && p==0)e.valid=0;
      if(!strcmp(fault,"scope") && i==0 && p==9)e.binding_set=0;
      printf("{\"valid\":%s",e.valid?"true":"false");
      string_field("source",e.source);string_field("vertex",e.vertex);string_field("fragment",e.fragment);
      string_field("wgsl_vertex",e.wgsl_vertex);string_field("wgsl_fragment",e.wgsl_fragment);
      printf(",\"binding_set\":%" PRId64 ",\"uses\":[",e.binding_set);
      int64_t n=aethyr_catalog_use_count(i,p);
      for(int64_t j=0;j<n;j++) {
        coax_t_RenderUse u=aethyr_catalog_use(i,p,j);
        if(!strcmp(fault,"slot") && i==0 && p==0 && j==0)u.slot++;
        if(!strcmp(fault,"kind") && i==0 && p==0 && j==0)u.native_kind=2;
        if(j)putchar(',');
        printf("{\"resource\":%" PRId64 ",\"slot\":%" PRId64 ",\"native_kind\":%" PRId64 ",\"shape\":%" PRId64 ",\"stride\":%" PRId64 ",\"stages\":%" PRId64 "}",u.resource,u.slot,u.native_kind,u.shape,u.stride,u.stages);
      }
      assert(aethyr_catalog_use(i,p,-1).slot==-1 && aethyr_catalog_use(i,p,n).slot==-1);
      printf("]}");
    }
    printf("]}");
  }
  puts("]");return 0;
}
