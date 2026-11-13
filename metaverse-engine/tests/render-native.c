// Independent finite metadata oracle. Arithmetic below uses sums in widened
// types rather than the product validator's remaining-capacity subtractions.
#include "render.h"
#include <assert.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

_Static_assert(sizeof(coax_t_RfDraw)==16,"four-u32 Draw ABI");
_Static_assert(offsetof(coax_t_RfDraw,first)==0 && offsetof(coax_t_RfDraw,count)==4 && offsetof(coax_t_RfDraw,base)==8 && offsetof(coax_t_RfDraw,instances)==12,"Draw order");
static unsigned effects;
void host_line(int64_t h,double a,double b,double c,double d,double e,double f,double g){(void)h;(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;(void)g;effects++;}
void host_dot(int64_t h,double a,double b,double c,double d,double e,double f){(void)h;(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;effects++;}
void host_slot(int64_t h,double a,double b,double c,double d,double e,double f,double g,double i){(void)h;(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;(void)g;(void)i;effects++;}
static int same(coax_t_RfFrame a,coax_t_RfFrame b){return a.active==b.active && a.epoch==b.epoch && a.passes==b.passes && a.bindings==b.bindings && a.draws==b.draws && a.words==b.words && a.limit.passes==b.limit.passes && a.limit.bindings==b.limit.bindings && a.limit.draws==b.limit.draws && a.limit.words==b.limit.words;}
int main(void){
    coax_t_RfLimits limits={4,8,8,64};coax_t_RfFrame f=render_begin(7,limits);
    coax_t_RfResource r[2]={{3,3,0,7,7},{1,1,16,1,9}};
    coax_t_RfHandle target={0,3};coax_t_RfBinding b={0,0,{1,1},0,16};
    coax_v_t_RfResource rs={r,2};coax_v_t_RfBinding bs={&b,1};coax_v_t_RfDraw empty={NULL,0};
    coax_t_RfGeometry geometry={5,7,3};uint64_t binding_cases=0,draw_cases=0;
    for(int kind=0;kind<=4;kind++)for(int generation=0;generation<=2;generation++)
    for(int group=-1;group<=4;group++)for(int binding=-1;binding<=8;binding++)
    for(int offset=-1;offset<=17;offset++)for(int words=0;words<=17;words++){
        r[1].kind=kind;b.resource.generation=generation;b.group=group;b.binding=binding;b.offset=offset;b.words=words;
        int expected=(kind==1 || kind==2) && generation==1 && group>=0 && group<4 && binding>=0 && binding<8 && offset>=0 && words>0 && offset+words<=16;
        coax_t_RfResult a=render_pass(f,rs,target,bs,empty,geometry);
        assert(a.accepted==expected);
        if(!expected)assert(same(a.frame,f));else assert(a.frame.passes==1 && a.frame.bindings==1 && a.frame.words==words);
        binding_cases++;
    }
    r[1]=(coax_t_RfResource){1,1,16,1,9};b=(coax_t_RfBinding){0,0,{1,1},0,16};
    uint32_t values[]={0,1,3,5,7,UINT32_MAX};coax_t_RfDraw d;
    for(unsigned direct=0;direct<2;direct++)for(unsigned i=0;i<6;i++)for(unsigned j=0;j<6;j++)for(unsigned k=0;k<6;k++)for(unsigned n=0;n<6;n++){
        uint32_t base=values[k]&UINT32_C(0x7fffffff);d=(coax_t_RfDraw){values[i],values[j],base|(direct?UINT32_C(0x80000000):0),values[n]};
        int expected=(uint64_t)d.first+d.count<=(direct?5:7) && (uint64_t)base+d.instances<=3;
        coax_t_RfResult a=render_pass(f,rs,target,bs,(coax_v_t_RfDraw){&d,1},geometry);
        assert(a.accepted==expected);if(!expected)assert(same(a.frame,f));draw_cases++;
    }
    for(int slot=-1;slot<=2;slot++)for(int generation=0;generation<=4;generation++)for(int first=6;first<=8;first++)for(int last=6;last<=8;last++){
        r[0].first=first;r[0].last=last;coax_t_RfHandle h={slot,generation};
        int expected=slot==0 && generation==3 && first<=7 && last>=7;
        coax_t_RfResult a=render_pass(f,rs,h,bs,empty,geometry);assert(a.accepted==expected);if(!expected)assert(same(a.frame,f));
    }
    r[0]=(coax_t_RfResource){3,3,0,7,7};
    coax_t_RfResult accepted=render_pass(f,rs,target,bs,empty,geometry);assert(accepted.accepted);
    coax_t_RfFrame done=render_finish(accepted.frame);assert(!done.active && done.epoch==7 && done.passes==0 && done.bindings==0 && done.draws==0 && done.words==0);
    assert(same(done,render_finish(done)));assert(!render_pass(done,rs,target,bs,empty,geometry).accepted);
    float screens[]={0,-1,NAN,INFINITY,1,390};float matrix[18];
    for(unsigned i=0;i<6;i++)for(unsigned j=0;j<6;j++)for(int n=15;n<=17;n++){
        float w=screens[i],h=screens[j];int expected=isfinite(w)&&isfinite(h)&&w>0&&h>0&&n>=16;
        assert(render_view(w,h,n)==expected);for(int q=0;q<18;q++)matrix[q]=19;
        assert(verse_grid_view(0,2,-4,0,1,0,w,h,(coax_v_f32){matrix,n})==expected);
        assert(matrix[16]==19 && matrix[17]==19);if(!expected)for(int q=0;q<18;q++)assert(matrix[q]==19);
    }
    assert(effects==0);
    printf("render admission: %llu binding cases, %llu draw cases, 180 lifetime cases; Grid108 viewport/extent cases; Draw16/order; refusal+cleanup preserved; host effects0\n",(unsigned long long)binding_cases,(unsigned long long)draw_cases);
    return 0;
}
