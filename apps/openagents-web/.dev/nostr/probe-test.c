// Explicit native test adapter, never linked into the development app.
#include "probe.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
extern void nv_run(int64_t mode,coax_v_s64 observation);
static coax_t_NostrProbe state;
static float vertices[50400*16],uniforms[16];
static coax_v_t_NostrProbe p(void){return(coax_v_t_NostrProbe){&state,1};}
static int errors[5]={0,7,7,8,2};
static void render(float w,float h,float d){
  int64_t mode=np_requested(p());
  if(mode>=0){int64_t result[4]={0};nv_run(mode,(coax_v_s64){result,4});np_observed(p(),(coax_v_s64){result,4});}
  assert(np_frame(p(),(coax_v_f32){vertices,50400*16},(coax_v_f32){uniforms,16},w,h,d)>0);
  assert(np_requested(p())==-1);assert(state.error==errors[state.mode]);
  assert(state.verified==(state.mode==0));assert((state.written>0)==(state.mode==0));
}
int main(void){
  // Confirm the UI does not react to input before it has a valid viewport.
  assert(!np_pointer(p(),0,0,0));assert(!np_key(p(),48));assert(!np_select(p(),-1));
  for(int i=-1;i<=5;i++){
    int64_t result[6]={165,0,0,0,0,165};
    nv_run(i,(coax_v_s64){result+1,4});
    assert(result[0]==165&&result[5]==165);
    assert(result[1]==(i<0||i>4?3:errors[i]));assert(result[4]==(i==0));
  }
  // Wrong observation shapes are ignored rather than inventing a result.
  int64_t short_result[3]={0};np_observed(p(),(coax_v_s64){short_result,3});assert(!state.initialized);
  render(1280,800,1);
  for(int i=0;i<5;i++){assert(np_key(p(),49+i));render(1280,800,1);assert(state.mode==i);}
  const float widths[3]={1280,390,780},heights[3]={800,844,1688},ratios[3]={1,1,2};
  for(int viewport=0;viewport<3;viewport++){
    float w=widths[viewport],h=heights[viewport],d=ratios[viewport];render(w,h,d);
    float first=(w/d-244)/2+22;
    for(int i=0;i<5;i++){
      float x=(first+i*50)*d,y=h-34*d;
      assert(np_pointer(p(),x,y,0));assert(np_pointer(p(),x,y,2));render(w,h,d);assert(state.mode==i);
    }
    int64_t previous=state.mode;
    assert(np_pointer(p(),first*d,h-34*d,0));assert(!np_pointer(p(),first*d,h-34*d,3));
    np_pointer(p(),first*d,h-34*d,2);render(w,h,d);assert(state.mode==previous);
    assert(np_pointer(p(),first*d,h-34*d,0));np_pointer(p(),(first+50)*d,h-34*d,2);
    render(w,h,d);assert(state.mode==previous);
  }
  puts("native: seven verifier cases; five keyboard cases; fifteen HUD clicks; six cancel/mismatch cases passed");
}
