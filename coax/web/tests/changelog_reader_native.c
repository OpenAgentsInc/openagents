/* Independent native geometry/input oracle for the product Coax reader. */
#include "changelog.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
enum { VERTICES=100000, STRIDE=12, UNIFORMS=88 };
static void guards(const unsigned char *p,size_t n){for(size_t i=0;i<n;i++)assert(p[i]==165);}
static void equal_route(coax_t_ChangelogState *s,const char *want){assert(s->action==1);assert((size_t)s->out_n==strlen(want));assert(!memcmp(s->outbox.e,want,strlen(want)));}
int main(int argc,char **argv){
  assert(argc==3);int first_release=atoi(argv[2]);assert(first_release>=0);FILE *source=fopen(argv[1],"rb");assert(source);assert(!fseek(source,0,SEEK_END));long bytes=ftell(source);assert(bytes>0&&bytes<=262144);rewind(source);char *markdown=malloc((size_t)bytes);assert(markdown);assert(fread(markdown,1,(size_t)bytes,source)==(size_t)bytes);assert(!fclose(source));
  size_t sn=sizeof(coax_t_ChangelogState),vn=VERTICES*STRIDE*sizeof(float),un=UNIFORMS*sizeof(float);
  unsigned char *sm=calloc(1,sn+32),*vm=malloc(vn+32),*um=malloc(un+32);assert(sm&&vm&&um);
  coax_t_ChangelogState *state=(void*)(sm+16);float *v=(void*)(vm+16),*u=(void*)(um+16);
  coax_v_t_ChangelogState s={state,1};coax_v_f32 vertices={v,VERTICES*STRIDE},uniforms={u,UNIFORMS};
  const float sizes[][3]={{320,568,1},{390,844,1},{1440,900,1},{390,844,2},{768,1024,3}};
  int64_t peak=0;int frames=0;int64_t first_css[3]={0},last_css[3]={0};
  for(size_t q=0;q<sizeof sizes/sizeof sizes[0];q++){
    memset(state,0,sn);memset(sm,165,16);memset(sm+16+sn,165,16);memset(vm,165,16);memset(vm+16+vn,165,16);memset(um,165,16);memset(um+16+un,165,16);
    float w=sizes[q][0],h=sizes[q][1],d=sizes[q][2];
    for(int position=0;position<3;position++){
      if(position==1)cc_wheel(s,12345);if(position==2)assert(cc_key(s,-9,0));
      float wanted_scroll=state->scroll;
      int64_t n=cc_frame(s,vertices,uniforms,w*d,h*d,d,.016f,1);assert(n>100&&n<=VERTICES);if(n>peak)peak=n;
      for(int64_t i=0;i<n*STRIDE;i++)assert(isfinite(v[i]));for(int i=0;i<UNIFORMS;i++)assert(isfinite(u[i]));assert(u[0]==w*d&&u[1]==h*d);
      assert(state->first>=0&&state->last>=state->first);assert(state->scroll>=0&&state->scroll<=state->limit);if(position==2)assert(state->scroll==state->limit&&state->first>0);
      if(position==0)assert(state->first==first_release);
      else assert(state->scroll==wanted_scroll);
      if(q==1){first_css[position]=state->first;last_css[position]=state->last;}if(q==3){assert(state->first==first_css[position]&&state->last==last_css[position]);}
      guards(sm,16);guards(sm+16+sn,16);guards(vm,16);guards(vm+16+vn,16);guards(um,16);guards(um+16+un,16);frames++;
    }
    for(int control=0;control<4;control++){
      coax_t_CcBox b=cc_control(s,control);assert(b.x0>=16&&b.x1<=w-16&&b.y1-b.y0>=44);float x=(b.x0+b.x1)*.5f*d,y=(b.y0+b.y1)*.5f*d;
      assert(cc_pointer(s,x,y,0,1,3));assert(cc_pointer(s,x,y,2,1,3));
      if(control==0)equal_route(state,"/projects/coax");if(control==2)equal_route(state,"/coax/changelog/index.md");if(control==3){assert(state->action==2&&state->out_n==bytes);assert(!memcmp(state->outbox.e,markdown,(size_t)bytes));}cc_done(s);
      assert(cc_pointer(s,x,y,0,1,3));assert(cc_pointer(s,x,y+30*d,1,1,3));assert(cc_pointer(s,x,y+30*d,2,1,3));assert(state->action==0);
    }
    state->focus=-1;assert(cc_key(s,-15,0)&&state->focus==0);assert(cc_key(s,-15,1)&&state->focus>=4);assert(cc_key(s,-2,0)&&state->action==1);assert(state->out_n>8&&!memcmp(state->outbox.e,"https://",8));cc_done(s);
    float tiny[12]={0};assert(cc_frame(s,(coax_v_f32){tiny,12},uniforms,w*d,h*d,d,0,1)<=1);assert(cc_frame(s,vertices,(coax_v_f32){u,87},w*d,h*d,d,0,1)==0);assert(cc_frame(s,vertices,uniforms,NAN,h*d,d,0,1)==0);
  }
  float previous_limit=state->limit;int64_t resized=cc_frame(s,vertices,uniforms,768*3,1224*3,3,0,1);assert(resized>0&&resized<=VERTICES);assert(state->limit<=previous_limit&&state->scroll<=state->limit);frames++;
  printf("{\"viewports\":5,\"frames\":%d,\"peak_vertices\":%lld,\"guard_bytes\":true,\"finite_geometry\":true,\"copy_bytes\":%ld}\n",frames,(long long)peak,bytes);
  free(markdown);free(sm);free(vm);free(um);return 0;
}
