/* Independent oracle for training-dense-f32-v1. This is verification code,
   never a product kernel. Calls the actual Coax-generated checked API and
   compares it with three ordinary scalar loops written from Y = X W and
   d<XW,DY>/dX,dW. No GPU execution or throughput claim is made here. */
#include "training-dense.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { float *allocation, *v; int64_t count; } Buffer;
static const float guard = 12345.5f;
static int observations;
#define REQUIRE(x) do { if (!(x)) { fprintf(stderr,"line %d: %s\n",__LINE__,#x); exit(1); } } while (0)
static Buffer buffer(int64_t count) {
    Buffer b = { .allocation = malloc((size_t)(count + 2) * sizeof(float)), .count = count };
    REQUIRE(b.allocation != NULL); b.v = b.allocation + 1;
    for (int64_t i = -1; i <= count; ++i) b.v[i] = guard;
    return b;
}
static coax_v_f32 view(Buffer b) { return (coax_v_f32){b.v,b.count}; }
static int same(float a,float b) { return memcmp(&a,&b,sizeof a) == 0; }
static void guards(Buffer b) { REQUIRE(same(b.v[-1],guard) && same(b.v[b.count],guard)); }
static void unchanged(Buffer b) { guards(b); for(int64_t i=0;i<b.count;i++) REQUIRE(same(b.v[i],guard)); }
static void release(Buffer b) { free(b.allocation); }
static float datum(int64_t i,int salt) { return (float)((i * (salt + 2) + salt) % 19 - 9) * 0.125f; }

static void reference(int m,int n,int k,const float *x,const float *w,const float *dy,float *y,float *dx,float *dw) {
    for(int row=0;row<m;row++) for(int col=0;col<n;col++) {
        float sum=0; for(int inner=0;inner<k;inner++) sum += x[row*k+inner] * w[inner*n+col];
        y[row*n+col]=sum;
    }
    for(int row=0;row<m;row++) for(int inner=0;inner<k;inner++) {
        float sum=0; for(int col=0;col<n;col++) sum += dy[row*n+col] * w[inner*n+col];
        dx[row*k+inner]=sum;
    }
    for(int inner=0;inner<k;inner++) for(int col=0;col<n;col++) {
        float sum=0; for(int row=0;row<m;row++) sum += x[row*k+inner] * dy[row*n+col];
        dw[inner*n+col]=sum;
    }
}
static void fixture(int m,int n,int k,int boundary) {
    Buffer x=buffer((int64_t)m*k),w=buffer((int64_t)k*n),dy=buffer((int64_t)m*n);
    Buffer y=buffer((int64_t)m*n),dx=buffer((int64_t)m*k),dw=buffer((int64_t)k*n);
    float *rx=malloc((size_t)x.count*4),*rw=malloc((size_t)w.count*4),*ry=malloc((size_t)y.count*4);
    REQUIRE(rx && rw && ry);
    for(int64_t i=0;i<x.count;i++) x.v[i]=boundary==1 ? (i%2 ? 1024.0f : -1024.0f) : boundary==2 ? (float)(i%7-3)*0.1f : datum(i,1);
    for(int64_t i=0;i<w.count;i++) w.v[i]=boundary==1 ? 1024.0f : boundary==2 ? (float)(i%11-5)*0.03f : datum(i,5);
    for(int64_t i=0;i<dy.count;i++) dy.v[i]=boundary==1 ? -1024.0f : boundary==2 ? (float)(i%13-6)*0.07f : datum(i,9);
    uint64_t before=0,after=0;
    for(int64_t i=0;i<x.count;i++) {uint32_t bits;memcpy(&bits,x.v+i,4);before=before*1099511628211ULL+bits;}
    for(int64_t i=0;i<w.count;i++) {uint32_t bits;memcpy(&bits,w.v+i,4);before=before*1099511628211ULL+bits;}
    for(int64_t i=0;i<dy.count;i++) {uint32_t bits;memcpy(&bits,dy.v+i,4);before=before*1099511628211ULL+bits;}
    coax_t_GdPlan p=gd_plan(m,n,k,x.count,w.count,dy.count,y.count,dx.count,dw.count,0);
    REQUIRE(p.status==COAX_GdStatus_ok);
    REQUIRE(p.m==m && p.n==n && p.k==k);
    REQUIRE(p.forward==((m+31)/32)*((n+31)/32));
    REQUIRE(p.dx==((m+31)/32)*((k+31)/32) && p.dw==((k+31)/32)*((n+31)/32));
    REQUIRE(p.bytes==8*((int64_t)m*k+(int64_t)k*n+(int64_t)m*n));
    REQUIRE(gd_run(m,n,k,view(x),view(w),view(dy),view(y),view(dx),view(dw),0)==COAX_GdStatus_ok);
    reference(m,n,k,x.v,w.v,dy.v,ry,rx,rw);
    for(int64_t i=0;i<y.count;i++) REQUIRE(same(y.v[i],ry[i]));
    for(int64_t i=0;i<dx.count;i++) REQUIRE(same(dx.v[i],rx[i]));
    for(int64_t i=0;i<dw.count;i++) REQUIRE(same(dw.v[i],rw[i]));
    guards(x);guards(w);guards(dy);guards(y);guards(dx);guards(dw);
    for(int64_t i=0;i<x.count;i++) {uint32_t bits;memcpy(&bits,x.v+i,4);after=after*1099511628211ULL+bits;}
    for(int64_t i=0;i<w.count;i++) {uint32_t bits;memcpy(&bits,w.v+i,4);after=after*1099511628211ULL+bits;}
    for(int64_t i=0;i<dy.count;i++) {uint32_t bits;memcpy(&bits,dy.v+i,4);after=after*1099511628211ULL+bits;}
    REQUIRE(before==after);
    printf("exact forward+dX+dW %dx%dx%d%s: %lld words, guards and inputs unchanged\n",m,n,k,boundary==1?" numeric-boundary":boundary==2?" rounded-products":"",(long long)(y.count+dx.count+dw.count));
    observations++; free(rx);free(rw);free(ry);release(x);release(w);release(dy);release(y);release(dx);release(dw);
}

static void refusals(void) {
    Buffer x=buffer(6),w=buffer(12),dy=buffer(8),y=buffer(8),dx=buffer(6),dw=buffer(12);
    for(int64_t i=0;i<x.count;i++)x.v[i]=1;
    for(int64_t i=0;i<w.count;i++)w.v[i]=1;
    for(int64_t i=0;i<dy.count;i++)dy.v[i]=1;
    for(int which=0;which<6;which++) {
        coax_v_f32 views[]={view(x),view(w),view(dy),view(y),view(dx),view(dw)};
        views[which].count--;
        REQUIRE(gd_run(2,4,3,views[0],views[1],views[2],views[3],views[4],views[5],0)==COAX_GdStatus_capacity);
        unchanged(y);unchanged(dx);unchanged(dw);observations++;
        views[which].count+=2;
        REQUIRE(gd_run(2,4,3,views[0],views[1],views[2],views[3],views[4],views[5],0)==COAX_GdStatus_capacity);
        unchanged(y);unchanged(dx);unchanged(dw);observations++;
    }
    REQUIRE(gd_run(2,4,3,view(x),view(w),view(dy),view(y),view(dx),view(dw),1)==COAX_GdStatus_alias);
    unchanged(y);unchanged(dx);unchanged(dw);observations++;
    const float bad[]={NAN,INFINITY,-INFINITY,1024.0001220703125f,-1024.0001220703125f};
    Buffer inputs[]={x,w,dy};
    for(int j=0;j<3;j++) for(size_t k=0;k<sizeof bad/sizeof *bad;k++) {
        inputs[j].v[inputs[j].count-1]=bad[k];
        REQUIRE(gd_run(2,4,3,view(x),view(w),view(dy),view(y),view(dx),view(dw),0)==COAX_GdStatus_number);
        unchanged(y);unchanged(dx);unchanged(dw);observations++;inputs[j].v[inputs[j].count-1]=1;
    }
    for(int j=0;j<3;j++) for(int invalid=0;invalid<2;invalid++) {
        int dims[]={2,4,3};dims[j]=invalid?129:0;
        REQUIRE(gd_run(dims[0],dims[1],dims[2],view(x),view(w),view(dy),view(y),view(dx),view(dw),0)==COAX_GdStatus_shape);
        unchanged(y);unchanged(dx);unchanged(dw);observations++;
    }
    release(x);release(w);release(dy);release(y);release(dx);release(dw);
    puts("bounded refusals: all output words and guards unchanged");
}
/* Numerical directional derivative of an independently evaluated real
   bilinear objective, not a finite difference through rounded f32 code. */
static double objective(const double *x,const double *w,const double *dy) {
    double s=0;for(int row=0;row<2;row++)for(int col=0;col<4;col++)for(int q=0;q<3;q++)s+=x[row*3+q]*w[q*4+col]*dy[row*4+col];return s;
}
static void derivative(void) {
    float x[6],w[12],dy[8],y[8],dx[6],dw[12];double xx[6],ww[12],dd[8];
    for(int i=0;i<6;i++)xx[i]=x[i]=datum(i,1);
    for(int i=0;i<12;i++)ww[i]=w[i]=datum(i,5);
    for(int i=0;i<8;i++)dd[i]=dy[i]=datum(i,9);
    REQUIRE(gd_run(2,4,3,(coax_v_f32){x,6},(coax_v_f32){w,12},(coax_v_f32){dy,8},(coax_v_f32){y,8},(coax_v_f32){dx,6},(coax_v_f32){dw,12},0)==COAX_GdStatus_ok);
    const double h=1.0/1024;
    for(int i=0;i<6;i++){xx[i]+=h;double hi=objective(xx,ww,dd);xx[i]-=2*h;double lo=objective(xx,ww,dd);xx[i]+=h;REQUIRE((hi-lo)/(2*h)==dx[i]);observations++;}
    for(int i=0;i<12;i++){ww[i]+=h;double hi=objective(xx,ww,dd);ww[i]-=2*h;double lo=objective(xx,ww,dd);ww[i]+=h;REQUIRE((hi-lo)/(2*h)==dw[i]);observations++;}
    puts("independent bilinear central differences: all18 operand derivatives exact");
}
int main(void) {
    const int shapes[][3]={{1,1,1},{31,33,7},{32,32,32},{33,31,35},{37,45,70},{128,1,128},{64,65,64},{128,128,128}};
    for(size_t i=0;i<sizeof shapes/sizeof *shapes;i++)fixture(shapes[i][0],shapes[i][1],shapes[i][2],0);
    fixture(2,4,3,1);fixture(33,35,34,2);refusals();derivative();
    printf("training dense CPU oracle: PASS %d observations; no GPU execution, resident optimizer or throughput qualification\n",observations);
    return 0;
}
