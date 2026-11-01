/* Test-only independent triangle enumeration, including the constant-edge continuation. */
#include "ground-ray-native.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

typedef struct { double x,y,z; } P;
typedef struct { int hit; double t; } Hit;
static P sub(P a,P b){return(P){a.x-b.x,a.y-b.y,a.z-b.z};}
static P cross(P a,P b){return(P){a.y*b.z-a.z*b.y,a.z*b.x-a.x*b.z,a.x*b.y-a.y*b.x};}
static double dot(P a,P b){return a.x*b.x+a.y*b.y+a.z*b.z;}
static Hit triangle(P a,P b,P c,P o,P d,double far){
  P u=sub(b,a),v=sub(c,a),p=cross(d,v),s=sub(o,a),q=cross(s,u);
  double determinant=dot(u,p);
  if(determinant==0)return(Hit){0,0};
  double x=dot(s,p)/determinant,y=dot(d,q)/determinant,t=dot(v,q)/determinant;
  if(x<0||y<0||x+y>1||t<0||t>far)return(Hit){0,0};
  return(Hit){1,t};
}
static int clamp(int x,int n){return x<0?0:x>n?n:x;}
static Hit enumeration(coax_t_Ground g,const float *h,P o,P d,double far){
  double xs[5]={-10000,0,0,0,10000},zs[6]={-10000,0,0,0,0,10000};
  for(int i=0;i<3;++i)xs[i+1]=(float)(g.west+(float)i*g.cell);
  for(int j=0;j<4;++j)zs[j+1]=(float)(g.south+(float)j*g.cell);
  Hit best={0,0};
  for(int j=0;j<5;++j)for(int i=0;i<4;++i){
    int x0=clamp(i-1,2),x1=clamp(i,2),z0=clamp(j-1,3),z1=clamp(j,3);
    P a={xs[i],h[z0*3+x0],zs[j]},c={xs[i+1],h[z1*3+x1],zs[j+1]};
    P bs[2]={{xs[i+1],h[z0*3+x1],zs[j]},{xs[i],h[z1*3+x0],zs[j+1]}};
    for(int k=0;k<2;++k){Hit q=triangle(a,bs[k],c,o,d,far);if(q.hit&&(!best.hit||q.t<best.t))best=q;}
  }
  return best;
}
static uint32_t random_word(uint32_t *seed){*seed=*seed*1664525u+1013904223u;return *seed;}
static float unit(uint32_t *seed){return(float)(random_word(seed)>>8)/16777216.0f;}

int main(void){
  coax_t_Ground g={.n=2,.rows=3,.cell=0.1f,.west=100.1f,.south=-12.3f};
  uint32_t seed=81037; unsigned samples=0;
  for(int mesh=0;mesh<32;++mesh){
    float guarded[14],saved[14];guarded[0]=NAN;guarded[13]=-INFINITY;float *h=guarded+1;
    for(int i=0;i<12;++i)h[i]=(float)((int)(random_word(&seed)%71)-35)*0.0625f;
    memcpy(saved,guarded,sizeof guarded);
    for(int ray=0;ray<128;++ray){
      coax_t_WorldPoint o={g.west-0.2f+unit(&seed)*0.6f,-2+unit(&seed)*8,g.south-0.2f+unit(&seed)*0.7f};
      coax_t_WorldPoint d={(unit(&seed)-0.5f)*0.6f,(unit(&seed)-0.5f)*12,(unit(&seed)-0.5f)*0.7f};
      double far=ray%3==0?0.25:ray%3==1?0.75:1.5;
      P origin={o.x,o.y,o.z},direction={d.x,d.y,d.z};
      Hit expected=enumeration(g,h,origin,direction,far);
      Hit down=enumeration(g,h,(P){o.x,1000,o.z},(P){0,-1,0},2000);
      assert(down.hit);
      if(o.y<=1000-down.t)expected=(Hit){1,0};
      coax_t_GroundRay q=ground_ray_native(g,(coax_v_f32){h,12},o,d,far,32);
      if(q.status!=COAX_GroundRayStatus_ok||q.hit!=expected.hit||(q.hit&&fabs(q.t-expected.t)>2e-9*(1+fabs(expected.t)))){
        fprintf(stderr,"ground ray mismatch: mesh%d ray%d status%u actual%d %.17g expected%d %.17g\n",mesh,ray,(unsigned)q.status,q.hit,q.t,expected.hit,expected.t);
        return 1;
      }
      assert(q.visited>0&&q.visited<=32&&q.triangles>=q.visited&&q.triangles<=2*q.visited);
      if(q.hit){
        assert(q.point.x==(float)((double)o.x+(double)d.x*q.t));
        assert(q.point.y==(float)((double)o.y+(double)d.y*q.t));
        assert(q.point.z==(float)((double)o.z+(double)d.z*q.t));
        double n=(double)q.normal.x*q.normal.x+(double)q.normal.y*q.normal.y+(double)q.normal.z*q.normal.z;
        assert(fabs(n-1)<0.000001);
      }
      ++samples;
    }
    assert(memcmp(saved,guarded,sizeof guarded)==0);
  }
  g=(coax_t_Ground){.n=3,.rows=1,.cell=0.1f};
  float peak[8]={0,0,4,0,0,0,4,0};
  coax_t_GroundRay peak_hit=ground_ray_native(g,(coax_v_f32){peak,8},(coax_t_WorldPoint){0.01f,2,0.05f},(coax_t_WorldPoint){1,0,0},0.3,8);
  assert(peak_hit.status==COAX_GroundRayStatus_ok&&peak_hit.hit&&fabs(peak_hit.t-0.14)<2e-8);
  float flat[8]={0};
  coax_t_GroundRay shallow=ground_ray_native(g,(coax_v_f32){flat,8},(coax_t_WorldPoint){0.1f,1e-8f,0.05f},(coax_t_WorldPoint){1,-1e-8f,0},2,8);
  assert(shallow.status==COAX_GroundRayStatus_ok&&shallow.hit&&shallow.t==1);
  coax_t_GroundRay exhausted=ground_ray_native(g,(coax_v_f32){flat,8},(coax_t_WorldPoint){0.01f,2,0.05f},(coax_t_WorldPoint){1,0,0},0.3,1);
  assert(exhausted.status==COAX_GroundRayStatus_exhausted&&!exhausted.hit&&exhausted.t==0);
  coax_t_GroundRay short_view=ground_ray_native(g,(coax_v_f32){NULL,0},(coax_t_WorldPoint){0,1,0},(coax_t_WorldPoint){0,-1,0},1,1);
  assert(short_view.status==COAX_GroundRayStatus_terrain&&!short_view.hit);
  printf("ground ray native: %u independent extended-mesh rays, narrow peak, shallow hit and atomic refusals pass\n",samples);
  return 0;
}
