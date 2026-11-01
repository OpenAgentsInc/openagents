/* Independent native oracle for the generated Coax ground sampler. This is test tooling. */
#include "ground-native.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct { double x, y, z; } Vertex;
typedef struct { double y, gx, gz; } Plane;
static uint32_t words(float x) { uint32_t out; memcpy(&out, &x, sizeof out); return out; }

/* The plane through three mesh vertices, via a cross product, independently of the
   production sampler's height differences and of the Coax oracle's barycentric solve. */
static Plane plane(Vertex a, Vertex b, Vertex c, double x, double z) {
  double ax=b.x-a.x, ay=b.y-a.y, az=b.z-a.z;
  double bx=c.x-a.x, by=c.y-a.y, bz=c.z-a.z;
  double nx=ay*bz-az*by, ny=az*bx-ax*bz, nz=ax*by-ay*bx;
  assert(ny != 0);
  return (Plane){a.y-(nx*(x-a.x)+nz*(z-a.z))/ny, -nx/ny, -nz/ny};
}
static void empty(coax_t_GroundNativeSample q) {
  assert(!q.ok && q.height == 0 && q.gx == 0 && q.gz == 0);
  assert(!q.clamped_x && !q.clamped_z);
}

int main(void) {
  coax_t_Ground g; coax_default_t_Ground(&g);
  assert(g.n==64 && g.cell==1.5f && g.west==-48 && g.south==-48);
  assert(g.rim_from==32 && g.rim==0.022f && g.rows==0);
  assert(ground_native_points(g)==4225);
  g=(coax_t_Ground){.n=2,.rows=3,.cell=1.25f,.west=-2.5f,.south=3.75f};
  assert(ground_native_points(g)==12);
  uint32_t rng=81029; unsigned samples=0;
  for (int mesh=0; mesh<64; ++mesh) {
    g.cell=(mesh&1)?0.1f:1.25f; g.west=(mesh&1)?100.1f:-2.5f; g.south=(mesh&1)?-12.3f:3.75f;
    float guarded[14], original[14]; guarded[0]=NAN; guarded[13]=-INFINITY;
    float *h=guarded+1;
    for (int k=0;k<12;++k) { rng=rng*1664525u+1013904223u; h[k]=(float)((int)((rng>>8)%20001)-10000)/137.0f; }
    memcpy(original,guarded,sizeof guarded);
    for (int j=0;j<3;++j) for (int i=0;i<2;++i) {
      double x0=(float)(g.west+(float)i*g.cell), z0=(float)(g.south+(float)j*g.cell);
      double x1=(float)(g.west+(float)(i+1)*g.cell), z1=(float)(g.south+(float)(j+1)*g.cell);
      Vertex a={x0,h[j*3+i],z0}, c={x1,h[(j+1)*3+i+1],z1};
      for (int iz=0;iz<=16;++iz) for (int ix=0;ix<=16;++ix) {
        double fx=ix/16.0, fz=iz/16.0;
        float x=(float)(x0+fx*(x1-x0)), z=(float)(z0+fz*(z1-z0));
        double actual_fx=((double)x-x0)/(x1-x0), actual_fz=((double)z-z0)/(z1-z0);
        Vertex b=actual_fx>=actual_fz ? (Vertex){x1,h[j*3+i+1],z0} : (Vertex){x0,h[(j+1)*3+i],z1};
        Plane expected=plane(a,b,c,x,z);
        coax_t_GroundNativeSample q=ground_native_sample(g,(coax_v_f32){h,12},x,z);
        assert(q.ok && !q.clamped_x && !q.clamped_z);
        assert(fabs(q.height-expected.y)<0.00004);
        if (ix>0 && ix<16 && iz>0 && iz<16) {
          assert(fabs(q.gx-expected.gx)<0.00001+0.00000006*fabs(expected.gx));
          assert(fabs(q.gz-expected.gz)<0.00001+0.00000006*fabs(expected.gz));
          coax_t_GroundNormal normal=ground_native_normal(q.ok,q.gx,q.gz);
          double length=sqrt(1+expected.gx*expected.gx+expected.gz*expected.gz);
          assert(fabs(normal.x+expected.gx/length)<0.000001);
          assert(fabs(normal.y-1/length)<0.000001 && fabs(normal.z+expected.gz/length)<0.000001);
        }
        ++samples;
      }
    }
    for(int j=0;j<4;++j) for(int i=0;i<3;++i) {
      coax_t_GroundNativeSample q=ground_native_sample(g,(coax_v_f32){h,12},g.west+i*g.cell,g.south+j*g.cell);
      assert(q.ok && words(q.height)==words(h[j*3+i]));
    }
    assert(memcmp(original,guarded,sizeof guarded)==0);
  }
  g=(coax_t_Ground){.n=1,.cell=1};
  float h[4]={-0.0f,1,2,4};
  for(int j=0;j<2;++j) for(int i=0;i<2;++i) {
    coax_t_GroundNativeSample q=ground_native_sample(g,(coax_v_f32){h,4},(float)i,(float)j);
    assert(q.ok && words(q.height)==words(h[j*2+i]));
  }
  coax_t_GroundNativeSample extended=ground_native_sample(g,(coax_v_f32){h,4},-5,6);
  assert(extended.ok && extended.height==2 && extended.clamped_x && extended.clamped_z);
  for(int count=0;count<4;++count) empty(ground_native_sample(g,(coax_v_f32){h,count},0.5f,0.5f));
  empty(ground_native_sample(g,(coax_v_f32){NULL,0},0.5f,0.5f));
  for(int k=0;k<4;++k) {
    float old=h[k]; h[k]=NAN; empty(ground_native_sample(g,(coax_v_f32){h,4},-3,5)); h[k]=old;
  }
  float bad[]={NAN,INFINITY,-INFINITY,1000000,-1000000};
  for(unsigned k=0;k<sizeof bad/sizeof *bad;++k) {
    empty(ground_native_sample(g,(coax_v_f32){h,4},bad[k],0));
    empty(ground_native_sample(g,(coax_v_f32){h,4},0,bad[k]));
  }
  g.n=INT64_MAX; assert(ground_native_points(g)==0);
  empty(ground_native_sample(g,(coax_v_f32){NULL,0},0,0));
  g.n=1; g.rows=INT64_MAX; assert(ground_native_points(g)==0);
  g.rows=0; g.cell=0; assert(ground_native_points(g)==0);
  g.cell=NAN; assert(ground_native_points(g)==0);
  coax_t_GroundNormal refused=ground_native_normal(1,NAN,0);
  assert(refused.x==0 && refused.y==1 && refused.z==0);
  printf("ground native: %u triangle samples, exact vertices, unchanged input and malformed views pass\n",samples);
  return 0;
}
