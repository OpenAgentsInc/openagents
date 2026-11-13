/* Test-only independent oracle. Closest points use plane projection plus three
 * segment distances, not the production Voronoi partition. A swept sphere's
 * first contact is found by minimizing convex squared distance along its path,
 * then bisecting the first radius crossing. No production face/capsule roots
 * or candidate-cell traversal are reused. Exact tangencies have named fixtures. */
#include "ground-contact-native.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

typedef struct { double x,y,z; } P;
typedef struct { P a,b,c; } Triangle;
typedef struct { int hit; double t; } Hit;
static P add(P a,P b){return(P){a.x+b.x,a.y+b.y,a.z+b.z};}
static P sub(P a,P b){return(P){a.x-b.x,a.y-b.y,a.z-b.z};}
static P scale(P a,double s){return(P){a.x*s,a.y*s,a.z*s};}
static P cross(P a,P b){return(P){a.y*b.z-a.z*b.y,a.z*b.x-a.x*b.z,a.x*b.y-a.y*b.x};}
static double dot(P a,P b){return a.x*b.x+a.y*b.y+a.z*b.z;}
static int barycentric_xz(Triangle t,P p,double *s,double *u){
  P a=sub(t.b,t.a),b=sub(t.c,t.a),q=sub(p,t.a);
  double d=a.x*b.z-a.z*b.x;
  *s=(q.x*b.z-q.z*b.x)/d;*u=(a.x*q.z-a.z*q.x)/d;
  return *s>=0&&*u>=0&&*s+*u<=1;
}
static double segment_squared(P a,P b,P p){
  P d=sub(b,a);double t=dot(sub(p,a),d)/dot(d,d);
  if(t<0)t=0;if(t>1)t=1;
  P q=sub(p,add(a,scale(d,t)));return dot(q,q);
}
static double distance_squared(Triangle t,P p){
  P n=cross(sub(t.b,t.a),sub(t.c,t.a));
  double plane=dot(sub(p,t.a),n)/dot(n,n),s,u;
  P projection=sub(p,scale(n,plane));
  double result=INFINITY;
  if(barycentric_xz(t,projection,&s,&u))result=plane*plane*dot(n,n);
  result=fmin(result,segment_squared(t.a,t.b,p));
  result=fmin(result,segment_squared(t.b,t.c,p));
  return fmin(result,segment_squared(t.c,t.a,p));
}
static int clamp(int x,int n){return x<0?0:x>n?n:x;}
static void mesh(coax_t_Ground g,const float *h,Triangle out[40]){
  double xs[5]={g.west-2,0,0,0,g.west+2},zs[6]={g.south-2,0,0,0,0,g.south+2};
  for(int i=0;i<3;++i)xs[i+1]=(float)(g.west+(float)i*g.cell);
  for(int j=0;j<4;++j)zs[j+1]=(float)(g.south+(float)j*g.cell);
  int k=0;
  for(int j=0;j<5;++j)for(int i=0;i<4;++i){
    int x0=clamp(i-1,2),x1=clamp(i,2),z0=clamp(j-1,3),z1=clamp(j,3);
    P a={xs[i],h[z0*3+x0],zs[j]},d={xs[i+1],h[z1*3+x1],zs[j+1]};
    out[k++]=(Triangle){a,{xs[i+1],h[z0*3+x1],zs[j]},d};
    out[k++]=(Triangle){a,{xs[i],h[z1*3+x0],zs[j+1]},d};
  }
  assert(k==40);
}
static int solid(Triangle t[40],P p){
  for(int i=0;i<40;++i){double s,u;if(barycentric_xz(t[i],p,&s,&u)){
    double height=t[i].a.y+s*(t[i].b.y-t[i].a.y)+u*(t[i].c.y-t[i].a.y);
    if(p.y<=height)return 1;
  }}
  return 0;
}
static int overlap(Triangle t[40],P p,double r){
  if(solid(t,p))return 1;
  for(int i=0;i<40;++i)if(distance_squared(t[i],p)<r*r)return 1;
  return 0;
}
static Hit swept_triangle(Triangle t,P p,P by,double r){
  double rr=r*r;
  if(distance_squared(t,p)<=rr)return(Hit){1,0};
  double left=0,right=1;
  for(int i=0;i<72;++i){
    double a=left+(right-left)/3,b=right-(right-left)/3;
    if(distance_squared(t,add(p,scale(by,a)))<distance_squared(t,add(p,scale(by,b))))right=b;
    else left=a;
  }
  double minimum=(left+right)/2;
  if(distance_squared(t,add(p,scale(by,minimum)))>rr)return(Hit){0,0};
  left=0;right=minimum;
  for(int i=0;i<64;++i){double mid=(left+right)/2;
    if(distance_squared(t,add(p,scale(by,mid)))<=rr)right=mid;else left=mid;
  }
  return(Hit){1,right};
}
static Hit swept(Triangle t[40],P p,P by,double r){
  if(solid(t,p))return(Hit){1,0};
  Hit best={0,0};
  for(int i=0;i<40;++i){Hit q=swept_triangle(t[i],p,by,r);if(q.hit&&(!best.hit||q.t<best.t))best=q;}
  return best;
}
static uint32_t random_word(uint32_t *seed){*seed=*seed*1664525u+1013904223u;return *seed;}
static float unit(uint32_t *seed){return(float)(random_word(seed)>>8)/16777216.0f;}
static void result_shape(coax_t_GroundContact q,coax_t_WorldPoint p,coax_t_WorldPoint by){
  assert(q.cells>0&&q.cells<=64&&q.triangles>=0&&q.triangles<=2*q.cells);
  if(!q.hit)return;
  assert(q.t>=0&&q.t<=1);
  assert(q.point.x==(float)((double)p.x+(double)by.x*q.t));
  assert(q.point.y==(float)((double)p.y+(double)by.y*q.t));
  assert(q.point.z==(float)((double)p.z+(double)by.z*q.t));
  double n=(double)q.normal.x*q.normal.x+(double)q.normal.y*q.normal.y+(double)q.normal.z*q.normal.z;
  assert(fabs(n-1)<0.000001);
}
int main(void){
  coax_t_Ground g={.n=2,.rows=3,.cell=0.1f,.west=100.1f,.south=-12.3f};
  uint32_t seed=17089;unsigned samples=0;
  for(int m=0;m<8;++m){
    float guarded[14],saved[14];guarded[0]=NAN;guarded[13]=-INFINITY;float *h=guarded+1;
    for(int i=0;i<12;++i)h[i]=(float)((int)(random_word(&seed)%17)-8)*0.0625f;
    memcpy(saved,guarded,sizeof guarded);Triangle triangles[40];mesh(g,h,triangles);
    for(int i=0;i<32;++i){
      coax_t_WorldPoint p={g.west-0.2f+unit(&seed)*0.6f,0.1f+unit(&seed)*1.1f,g.south-0.2f+unit(&seed)*0.7f};
      coax_t_WorldPoint by={(unit(&seed)-0.5f)*0.4f,-0.4f-unit(&seed)*1.5f,(unit(&seed)-0.5f)*0.4f};
      float r=0.01f+unit(&seed)*0.12f;P origin={p.x,p.y,p.z},motion={by.x,by.y,by.z};
      int expected_overlap=overlap(triangles,origin,r);Hit expected_sweep=swept(triangles,origin,motion,r);
      coax_t_GroundContact a=ground_overlap_native(g,(coax_v_f32){h,12},p,r,64);
      coax_t_GroundContact b=ground_sweep_native(g,(coax_v_f32){h,12},p,r,by,64);
      if(a.status!=COAX_GroundRayStatus_ok||a.hit!=expected_overlap||b.status!=COAX_GroundRayStatus_ok||b.hit!=expected_sweep.hit||(b.hit&&fabs(b.t-expected_sweep.t)>2e-7)){
        fprintf(stderr,"ground contact mismatch mesh%d sample%d: overlap %u/%d expected%d; sweep %u/%d %.17g expected%d %.17g\n",m,i,(unsigned)a.status,a.hit,expected_overlap,(unsigned)b.status,b.hit,b.t,expected_sweep.hit,expected_sweep.t);return 1;
      }
      result_shape(a,p,(coax_t_WorldPoint){0});result_shape(b,p,by);++samples;
    }
    assert(memcmp(saved,guarded,sizeof guarded)==0);
  }
  g=(coax_t_Ground){.n=1,.cell=1};float ridge[4]={1,0,0,1};
  coax_t_GroundContact ridge_hit=ground_sweep_native(g,(coax_v_f32){ridge,4},(coax_t_WorldPoint){.5f,3,.5f},.25f,(coax_t_WorldPoint){0,-3,0},8);
  assert(ridge_hit.status==COAX_GroundRayStatus_ok&&ridge_hit.hit&&fabs(ridge_hit.t-7.0/12)<1e-12&&ridge_hit.feature==10);
  float flat[4]={0};
  coax_t_GroundContact touch=ground_sweep_native(g,(coax_v_f32){flat,4},(coax_t_WorldPoint){.5f,.25f,.5f},.25f,(coax_t_WorldPoint){.25f,0,0},8);
  assert(touch.status==COAX_GroundRayStatus_ok&&touch.hit&&touch.t==0&&touch.normal.x==-1);
  coax_t_GroundContact still=ground_overlap_native(g,(coax_v_f32){flat,4},(coax_t_WorldPoint){.5f,.25f,.5f},.25f,8);
  assert(still.status==COAX_GroundRayStatus_ok&&!still.hit);
  coax_t_GroundContact exhausted=ground_overlap_native(g,(coax_v_f32){flat,4},(coax_t_WorldPoint){1.2f,4,.5f},.3f,1);
  assert(exhausted.status==COAX_GroundRayStatus_exhausted&&!exhausted.hit&&exhausted.t==0);
  coax_t_GroundContact short_view=ground_overlap_native(g,(coax_v_f32){NULL,0},(coax_t_WorldPoint){0,1,0},.1f,8);
  assert(short_view.status==COAX_GroundRayStatus_terrain&&!short_view.hit);
  printf("ground contact native: %u independent overlaps and convex-distance sweeps, ridge/touch and atomic refusals pass\n",samples);
  return 0;
}
