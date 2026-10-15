// Float32 encoder and digit kernels. No fast math or reduced-precision matrices.
#include <metal_stdlib>
using namespace metal;

kernel void small_embed(device const uchar* table [[buffer(0)]], device const int* ids [[buffer(1)]],
                        device float* out [[buffer(2)]],constant uint* a [[buffer(8)]],constant float* f [[buffer(9)]],uint i [[thread_position_in_grid]]) {
    uint n=a[0], h=a[1]; if(i>=n*h)return;
    int id=ids[i/h]; out[i]=id>=0 && uint(id)<a[2] ? (float(table[ulong(id)*h+i%h])-float(a[3]))*f[0] : 0.0f;
}

kernel void small_norm(device const float* input [[buffer(0)]],device const float* weight [[buffer(1)]],
                       device float* out [[buffer(2)]],constant uint* a [[buffer(8)]],constant float* f [[buffer(9)]],
                       uint row [[threadgroup_position_in_grid]],uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float reduction[256]; uint h=a[1];
    float sum=0;for(uint c=tid;c<h;c+=256)sum+=input[row*h+c];
    reduction[tid]=sum;threadgroup_barrier(mem_flags::mem_threadgroup);
    for(uint step=128;step;step/=2){if(tid<step)reduction[tid]+=reduction[tid+step];threadgroup_barrier(mem_flags::mem_threadgroup);}
    float mean=reduction[0]/float(h);threadgroup_barrier(mem_flags::mem_threadgroup);
    float variance=0;for(uint c=tid;c<h;c+=256){float v=input[row*h+c]-mean;variance=fma(v,v,variance);}
    reduction[tid]=variance;threadgroup_barrier(mem_flags::mem_threadgroup);
    for(uint step=128;step;step/=2){if(tid<step)reduction[tid]+=reduction[tid+step];threadgroup_barrier(mem_flags::mem_threadgroup);}
    float inv=rsqrt(reduction[0]/float(h)+f[0]);threadgroup_barrier(mem_flags::mem_threadgroup);
    for(uint c=tid;c<h;c+=256)out[row*h+c]=(input[row*h+c]-mean)*inv*weight[c];
}

kernel void small_matmul(device const float* left [[buffer(0)]],device const float* right [[buffer(1)]],
                         device float* out [[buffer(2)]],constant uint* a [[buffer(8)]],
                         uint2 group [[threadgroup_position_in_grid]],uint2 local [[thread_position_in_threadgroup]]) {
    threadgroup float av[16][16],bv[16][16];
    uint m=a[0],k=a[1],n=a[2],row=group.y*16+local.y,col=group.x*16+local.x;
    float sum=0;
    for(uint base=0;base<k;base+=16){
        av[local.y][local.x]=row<m && base+local.x<k ? left[row*k+base+local.x] : 0.0f;
        bv[local.y][local.x]=base+local.y<k && col<n ? right[(base+local.y)*n+col] : 0.0f;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for(uint t=0;t<16;++t)sum=fma(av[local.y][t],bv[t][local.x],sum);
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if(row<m && col<n)out[row*n+col]=sum;
}

kernel void small_rope(device const float* qkv [[buffer(0)]],device const float* freq [[buffer(1)]],device float* out [[buffer(2)]],
                       constant uint* a [[buffer(8)]],uint i [[thread_position_in_grid]]) {
    uint n=a[0],heads=a[1],d=a[2];if(i>=n*heads*d)return;
    uint c=i%d,t=(i/d)%n,head=i/(d*n),h=heads*d,half_d=d/2,j=c%half_d;
    float angle=float(t)*freq[j],s=sin(angle),co=cos(angle),scale=sqrt(rsqrt(float(d)));
    for(uint part=0;part<2;++part){uint row=t*3*h+part*h+head*d;
        float x=qkv[row+j],y=qkv[row+j+half_d];
        out[part*heads*n*d+i]=(c<half_d ? x*co-y*s : y*co+x*s)*scale;
    }
    out[2*heads*n*d+i]=qkv[t*3*h+2*h+head*d+c];
}

kernel void small_attention(device const float* qkv [[buffer(0)]],device const int* mask [[buffer(1)]],device float* out [[buffer(2)]],
    constant uint* a [[buffer(8)]],uint2 group [[threadgroup_position_in_grid]],uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float scores[512],reduction[256];
    uint n=a[0],heads=a[1],d=a[2],t=group.x,head=group.y,lane=tid%32,warp=tid/32;
    int radius=int(a[3]),first=radius<0 ? 0 : max(0,int(t)-radius),end=radius<0 ? int(n) : min(int(n),int(t)+radius+1);
    uint q=(head*n+t)*d,k=heads*n*d+head*n*d,v=2*heads*n*d+head*n*d;
    for(int key=first+int(warp);key<end;key+=8){float dot=0;
        for(uint c=lane;c<d;c+=32)dot=fma(qkv[q+c],qkv[k+uint(key)*d+c],dot);
        dot=simd_sum(dot);if(lane==0)scores[key]=mask[key] ? dot : -INFINITY;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float maximum=-INFINITY;for(int key=first+int(tid);key<end;key+=256)maximum=max(maximum,scores[key]);
    reduction[tid]=maximum;threadgroup_barrier(mem_flags::mem_threadgroup);
    for(uint step=128;step;step/=2){if(tid<step)reduction[tid]=max(reduction[tid],reduction[tid+step]);threadgroup_barrier(mem_flags::mem_threadgroup);}
    maximum=reduction[0];threadgroup_barrier(mem_flags::mem_threadgroup);
    float sum=0;for(int key=first+int(tid);key<end;key+=256){float value=maximum==-INFINITY ? 0.0f : exp(scores[key]-maximum);scores[key]=value;sum+=value;}
    reduction[tid]=sum;threadgroup_barrier(mem_flags::mem_threadgroup);
    for(uint step=128;step;step/=2){if(tid<step)reduction[tid]+=reduction[tid+step];threadgroup_barrier(mem_flags::mem_threadgroup);}
    float inv=reduction[0]>0 ? 1.0f/reduction[0] : 0.0f;threadgroup_barrier(mem_flags::mem_threadgroup);
    uint parts=256/d;float value=0;
    if(tid<parts*d){uint c=tid%d,part=tid/d;for(int key=first+int(part);key<end;key+=int(parts))value=fma(scores[key]*inv,qkv[v+uint(key)*d+c],value);}
    reduction[tid]=value;threadgroup_barrier(mem_flags::mem_threadgroup);
    if(tid<d){float total=0;for(uint part=0;part<parts;++part)total+=reduction[tid+part*d];out[t*heads*d+head*d+tid]=total;}
}

inline float small_erf(float x) {
    const float ax = fabs(x);
    float r;
    if (ax < 0.5f) {
        const float t = x * x;
        const float num = fma(fma(fma(fma(0.185777706184603153f, t, 3.16112374387056560f), t, 113.864154151050156f), t,
                                  377.485237685302021f), t, 3209.37758913846947f);
        const float den = fma(fma(fma(fma(1.0f, t, 23.6012909523441209f), t, 244.024637934444173f), t,
                                  1282.61652607737228f), t, 2844.23683343917062f);
        return x * num / den;
    } else if (ax < 4.0f) {
        const float num = fma(fma(fma(fma(fma(fma(fma(fma(2.15311535474403846e-8f, ax, 0.564188496988670089f), ax,
                                                          8.88314979438837594f), ax, 66.1191906371416295f), ax,
                                                  298.635138197400131f), ax, 881.952221241769090f), ax,
                                          1712.04761263407058f), ax, 2051.07837782607147f), ax, 1230.33935479799725f);
        const float den = fma(fma(fma(fma(fma(fma(fma(fma(1.0f, ax, 15.7449261107098347f), ax, 117.693950891312499f), ax,
                                                  537.181101862009858f), ax, 1621.38957456669019f), ax,
                                          3290.79923573345963f), ax, 4362.61909014324716f), ax, 3439.36767414372164f), ax,
                              1230.33935480374942f);
        r = 1.0f - exp(-ax * ax) * num / den;
    } else {
        r = 1.0f;
    }
    return x < 0.0f ? -r : r;
}


kernel void small_geglu(device const float* input [[buffer(0)]],device float* out [[buffer(1)]],constant uint* a [[buffer(8)]],uint i [[thread_position_in_grid]]) {
    uint f=a[1];if(i>=a[0]*f)return;uint row=i/f,c=i%f;float x=input[row*2*f+c];
    out[i]=0.5f*x*(1.0f+small_erf(x*0.70710678118654752f))*input[row*2*f+f+c];
}
kernel void small_add(device const float* input [[buffer(0)]],device float* out [[buffer(1)]],constant uint* a [[buffer(8)]],uint i [[thread_position_in_grid]]) {
    if(i<a[0])out[i]+=input[i];
}
kernel void small_bias(device float* out [[buffer(0)]],device const float* bias [[buffer(1)]],constant uint* a [[buffer(8)]],uint i [[thread_position_in_grid]]) {
    if(i<a[0])out[i]+=bias[i];
}
kernel void small_digit_conv_pool(device const float* input [[buffer(0)]],device const float* w [[buffer(1)]],device const float* bias [[buffer(2)]],device float* out [[buffer(3)]],constant uint* a [[buffer(8)]],uint i [[thread_position_in_grid]]) {
    uint channels=a[0],width=a[1],outputs=a[2],pool=a[3],next=width/pool;if(i>=outputs*next*next)return;
    uint oc=i/(next*next),y=(i/next)%next,x=i%next;float maximum=0;
    for(uint py=0;py<pool;++py)for(uint px=0;px<pool;++px){float sum=0;
        for(uint c=0;c<channels;++c)for(int ky=0;ky<5;++ky)for(int kx=0;kx<5;++kx){int iy=int(y*pool+py)+ky-2,ix=int(x*pool+px)+kx-2;
            if(iy>=0 && ix>=0 && uint(iy)<width && uint(ix)<width)sum=fma(input[(c*width+uint(iy))*width+uint(ix)],w[((oc*channels+c)*5+uint(ky))*5+uint(kx)],sum);
        }maximum=max(maximum,sum+bias[oc]);
    }out[i]=maximum;
}
kernel void small_digit_head(device const float* input [[buffer(0)]],device const float* w [[buffer(1)]],device const float* bias [[buffer(2)]],device float* out [[buffer(3)]],uint i [[thread_position_in_grid]]) {
    if(i>=10)return;float sum=0;for(uint k=0;k<256;++k)sum=fma(input[k],w[k*10+i],sum);out[i]=sum+bias[i];
}
