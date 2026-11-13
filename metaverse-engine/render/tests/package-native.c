/* Normal-C byte/field transport. Coax decides parsing/admission/hashes.
 * The Python qualifier inventories every exported record field. */
#include "package-native.h"
#include <assert.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned char *json_bytes;static size_t json_size;static int borrowed_ok=1;
static unsigned char *read_file(const char *path,size_t *size) {
  FILE *f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>=0);rewind(f);
  unsigned char *data=malloc((size_t)n+1);assert(data);assert(fread(data,1,(size_t)n,f)==(size_t)n);fclose(f);data[n]=0;*size=(size_t)n;return data;
}
static void string(coax_string s) {
  assert(s.count>=0);
  uintptr_t p=(uintptr_t)s.data,base=(uintptr_t)json_bytes,end=base+json_size;
  if(s.count && (p<base || p>end || (uint64_t)s.count>end-p))borrowed_ok=0;
  putchar('"');for(int64_t i=0;i<s.count;i++){unsigned char c=(unsigned char)s.data[i];
    if(c=='"'||c=='\\')printf("\\%c",c);else if(c<32)printf("\\u%04x",c);else putchar(c);
  }putchar('"');
}
static void entry(int index,coax_t_RenderPackageEntry e) {
  printf("{\"entry_index\":%d,\"present\":%s",index,e.present?"true":"false");if(!e.present){putchar('}');return;}
  printf(",\"binding_set\":%" PRId64 ",\"vertex\":",e.binding_set);string(e.vertex);
  printf(",\"fragment\":");string(e.fragment);printf(",\"wgsl_vertex\":");string(e.wgsl_vertex);printf(",\"wgsl_fragment\":");string(e.wgsl_fragment);
  printf(",\"spirv_file\":");string(e.spirv_file);printf(",\"spirv_sha256\":");string(e.spirv_sha256);
  printf(",\"tree_file\":");string(e.tree_file);printf(",\"tree_sha256\":");string(e.tree_sha256);
  coax_t_RenderBuildReflection r=e.reflection;
  printf(",\"native_int64\":%s,\"reflection_ok\":%s,\"reflection_schema\":%" PRId64 ",\"tree_version\":%" PRId64 ",\"derivatives\":%" PRId64 ",\"control\":%" PRId64,
    e.native_int64?"true":"false",r.ok?"true":"false",r.schema,r.tree_version,r.derivatives,r.control);
  printf(",\"use_count\":%" PRId64 ",\"uses\":[",r.use_count);assert(r.use_count>=0&&r.use_count<=32);
  for(int64_t i=0;i<r.use_count;i++){coax_t_RenderReflectedUse u=r.uses.e[i];if(i)putchar(',');printf("{\"name\":");string(u.name);
    printf(",\"slot\":%" PRId64 ",\"native_kind\":%" PRId64 ",\"shape\":%" PRId64 ",\"stride\":%" PRId64 ",\"stages\":%" PRId64 "}",u.slot,u.native_kind,u.shape,u.stride,u.stages);}
  printf("],\"vertex_stride\":%" PRId64 ",\"attribute_count\":%" PRId64 ",\"attributes\":[",r.vertex_stride,r.attribute_count);assert(r.attribute_count>=0&&r.attribute_count<=32);
  for(int64_t i=0;i<r.attribute_count;i++){coax_t_RenderVertexInput a=r.attributes.e[i];if(i)putchar(',');printf("{\"location\":%" PRId64 ",\"scalar\":%" PRId64 ",\"width\":%" PRId64 ",\"offset\":%" PRId64 "}",a.location,a.scalar,a.width,a.offset);}
  printf("],\"fragment_targets\":%" PRId64 ",\"fragment_depth\":%s}",r.fragment_targets,r.fragment_depth?"true":"false");
}
int main(int argc,char **argv) {
  assert(argc>=3);json_bytes=read_file(argv[1],&json_size);int profile=atoi(argv[2]);
  const char *fault=argc>3?argv[3]:"";coax_t_RenderPackageEntry guarded[16];memset(guarded,0xa5,sizeof guarded);
  int short_view=!strcmp(fault,"short");
  coax_t_RenderPackage p=render_pkg_native_parse((coax_string){(char*)json_bytes,(int64_t)json_size},(coax_v_t_RenderPackageEntry){guarded+1,short_view?13:14});
  unsigned char canary[sizeof(coax_t_RenderPackageEntry)];memset(canary,0xa5,sizeof canary);
  assert(!memcmp(guarded,canary,sizeof canary)&&!memcmp(guarded+15,canary,sizeof canary));
  if(p.status==COAX_RenderPackageStatus_ok){
    if(!strcmp(fault,"profile"))p.profile_index++;
    if(!strcmp(fault,"module_count"))p.module_count++;
    if(!strcmp(fault,"entry_count"))p.entry_count++;
    if(!strcmp(fault,"resource"))p.entries.data[0].reflection.uses.e[0].stages++;
    if(!strcmp(fault,"attribute"))p.entries.data[12].reflection.attributes.e[0].offset++;
    if(!strcmp(fault,"borrow"))p.modules.e[0].wgsl_file=(coax_string){"outside",7};
  }
  int backend=!strcmp(fault,"vulkan-no-int64")?2:1;
  int64_t admitted=render_pkg_native_admit(p,profile,backend,1,!strcmp(fault,"vulkan-no-int64")?0:1);
  printf("{\"parse\":%u,\"admit\":%" PRId64 ",\"entry_view_count\":%" PRId64 ",\"guards\":true",(unsigned)p.status,admitted,p.entries.count);
  if(p.status==COAX_RenderPackageStatus_ok){
    assert(p.entries.data==guarded+1&&p.entries.count==14);
    printf(",\"profile_index\":%" PRId64 ",\"slug\":",p.profile_index);string(p.slug);printf(",\"app\":");string(p.app);
    printf(",\"module_count\":%" PRId64 ",\"entry_count\":%" PRId64 ",\"modules\":[",p.module_count,p.entry_count);
    for(int i=0;i<3;i++){coax_t_RenderPackageModule m=p.modules.e[i];if(i)putchar(',');printf("{\"present\":%s",m.present?"true":"false");
      if(m.present){printf(",\"binding_set\":%" PRId64 ",\"wgsl_file\":",m.binding_set);string(m.wgsl_file);printf(",\"wgsl_sha256\":");string(m.wgsl_sha256);}putchar('}');}
    printf("],\"entries\":[");for(int i=0;i<14;i++){if(i)putchar(',');entry(i,p.entries.data[i]);}putchar(']');
  }
  if(argc>=7){size_t wn,sn;unsigned char *w=read_file(argv[4],&wn),*s=read_file(argv[5],&sn);int index=atoi(argv[6]),set=argc>=8?atoi(argv[7]):0;
    printf(",\"module_bytes\":%" PRId64 ",\"spirv_bytes\":%" PRId64,render_pkg_native_module(p,set,(coax_v_u8){w,(int64_t)wn}),render_pkg_native_spirv(p,index,(coax_v_u8){s,(int64_t)sn}));free(w);free(s);
    if(argc==9){size_t tn;unsigned char *t=read_file(argv[8],&tn);printf(",\"tree_bytes\":%" PRId64,render_pkg_native_tree(p,index,(coax_v_u8){t,(int64_t)tn}));free(t);}
  }
  printf(",\"borrowed_strings\":%s}\n",borrowed_ok?"true":"false");free(json_bytes);return 0;
}
