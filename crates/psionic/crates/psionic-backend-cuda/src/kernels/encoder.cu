// Device-resident bidirectional encoder operations. Float32 throughout.
#include <cuda_runtime.h>
#include <math_constants.h>

__global__ void encoder_embed_kernel(const unsigned char* table, const int* ids, float* out,
                      int n, int h, int vocab, int zero, float scale) {
    int i=blockIdx.x*blockDim.x+threadIdx.x;
    if(i<n*h) { int id=ids[i/h]; out[i]=id>=0 && id<vocab ? (float(table[static_cast<size_t>(id)*h+i%h])-float(zero))*scale : 0.0f; }
}
__global__ void encoder_rope_kernel(const float* qkv, const float* freq, float* out,
                     int n, int heads, int d) {
    int i=blockIdx.x*blockDim.x+threadIdx.x;
    if(i>=heads*n*d) return;
    int c=i%d, t=(i/d)%n, head=i/(d*n), h=heads*d;
    int half=d/2, j=c%half;
    float s,cosa; sincosf(float(t)*freq[j], &s, &cosa);
    float scale=sqrtf(1.0f/sqrtf(float(d)));
    for(int part=0;part<2;++part) {
        const float* row=qkv+t*3*h+part*h+head*d;
        float a=row[j], b=row[j+half];
        out[part*heads*n*d+i]=(c<half ? a*cosa-b*s : b*cosa+a*s)*scale;
    }
    out[2*heads*n*d+i]=qkv[t*3*h+2*h+head*d+c];
}
__device__ float encoder_warp_sum_kernel(float x) {
    for(int delta=16;delta;delta/=2) x+=__shfl_down_sync(0xffffffff,x,delta);
    return x;
}
__global__ void encoder_attention_kernel(const float* qkv, const int* mask, float* out,
                          int n, int heads, int d, int radius) {
    __shared__ float scores[512];
    __shared__ float reduction[256];
    int t=blockIdx.x, head=blockIdx.y, tid=threadIdx.x;
    int lane=tid%32, warp=tid/32;
    const float* q=qkv+(head*n+t)*d;
    const float* k=qkv+heads*n*d+head*n*d;
    const float* v=qkv+2*heads*n*d+head*n*d;
    int first=radius<0 ? 0 : max(0,t-radius);
    int end=radius<0 ? n : min(n,t+radius+1);
    for(int key=first+warp;key<end;key+=8) {
        float dot=0;
        for(int c=lane;c<d;c+=32) dot+=q[c]*k[key*d+c];
        dot=encoder_warp_sum_kernel(dot);
        if(lane==0) scores[key]=mask[key] ? dot : -CUDART_INF_F;
    }
    __syncthreads();
    float maximum=-CUDART_INF_F;
    for(int key=first+tid;key<end;key+=256) maximum=fmaxf(maximum,scores[key]);
    reduction[tid]=maximum; __syncthreads();
    for(int stride=128;stride;stride/=2) {
        if(tid<stride) reduction[tid]=fmaxf(reduction[tid],reduction[tid+stride]);
        __syncthreads();
    }
    maximum=reduction[0];
    __syncthreads();
    float sum=0;
    for(int key=first+tid;key<end;key+=256) {
        float value=maximum==-CUDART_INF_F ? 0.0f : expf(scores[key]-maximum);
        scores[key]=value; sum+=value;
    }
    reduction[tid]=sum; __syncthreads();
    for(int stride=128;stride;stride/=2) {
        if(tid<stride) reduction[tid]+=reduction[tid+stride];
        __syncthreads();
    }
    float inv=reduction[0]>0 ? 1.0f/reduction[0] : 0.0f;
    __syncthreads();
    // Split the value reduction across otherwise idle warps for narrow heads.
    if(d<=256) {
        int parts=256/d;
        float result=0;
        if(tid<parts*d) {
            int c=tid%d, part=tid/d;
            for(int key=first+part;key<end;key+=parts) result+=scores[key]*inv*v[key*d+c];
        }
        reduction[tid]=result; __syncthreads();
        if(tid<d) {
            float total=0;for(int part=0;part<parts;++part) total+=reduction[tid+part*d];
            out[t*heads*d+head*d+tid]=total;
        }
    } else {
        for(int c=tid;c<d;c+=256) {
            float result=0;
            for(int key=first;key<end;++key) result+=scores[key]*inv*v[key*d+c];
            out[t*heads*d+head*d+c]=result;
        }
    }
}
__global__ void encoder_geglu_kernel(const float* up,float* out,int n,int f) {
    int i=blockIdx.x*blockDim.x+threadIdx.x;
    if(i>=n*f) return;
    float x=up[(i/f)*2*f+i%f];
    out[i]=x*(1.0f+erff(x*0.7071067811865475244f))*0.5f*up[(i/f)*2*f+f+i%f];
}
extern "C" int psionic_encoder_embed(const void* table,const void* ids,void* out,int n,int h,int vocab,int zero,float scale,void* stream) {
    encoder_embed_kernel<<<(n*h+255)/256,256,0,(cudaStream_t)stream>>>((const unsigned char*)table,(const int*)ids,(float*)out,n,h,vocab,zero,scale);
    return int(cudaGetLastError());
}
extern "C" int psionic_encoder_rope(const void* qkv,const void* freq,void* out,int n,int heads,int d,void* stream) {
    encoder_rope_kernel<<<(n*heads*d+255)/256,256,0,(cudaStream_t)stream>>>((const float*)qkv,(const float*)freq,(float*)out,n,heads,d);
    return int(cudaGetLastError());
}
extern "C" int psionic_encoder_attention(const void* qkv,const void* mask,void* out,int n,int heads,int d,int radius,int queries,void* stream) {
    encoder_attention_kernel<<<dim3(queries,heads),256,0,(cudaStream_t)stream>>>((const float*)qkv,(const int*)mask,(float*)out,n,heads,d,radius);
    return int(cudaGetLastError());
}
extern "C" int psionic_encoder_geglu(const void* up,void* out,int n,int f,void* stream) {
    encoder_geglu_kernel<<<(n*f+255)/256,256,0,(cudaStream_t)stream>>>((const float*)up,(float*)out,n,f);
    return int(cudaGetLastError());
}

__global__ void encoder_masked_softmax_kernel(float* scores,const int* mask,int n,int radius) {
    __shared__ float reduction[256];
    int tid=threadIdx.x,t=blockIdx.x%n;
    float* row=scores+blockIdx.x*n;
    float maximum=-CUDART_INF_F;
    for(int key=tid;key<n;key+=256) {
        bool allowed=mask[key] && (radius<0 || abs(t-key)<=radius);
        float score=allowed ? row[key] : -CUDART_INF_F;
        row[key]=score;maximum=fmaxf(maximum,score);
    }
    reduction[tid]=maximum;__syncthreads();
    for(int stride=128;stride;stride/=2) {if(tid<stride) reduction[tid]=fmaxf(reduction[tid],reduction[tid+stride]);__syncthreads();}
    maximum=reduction[0];__syncthreads();
    float sum=0;
    for(int key=tid;key<n;key+=256) {float value=maximum==-CUDART_INF_F ? 0 : expf(row[key]-maximum);row[key]=value;sum+=value;}
    reduction[tid]=sum;__syncthreads();
    for(int stride=128;stride;stride/=2) {if(tid<stride) reduction[tid]+=reduction[tid+stride];__syncthreads();}
    float inv=reduction[0]>0 ? 1.0f/reduction[0] : 0;
    for(int key=tid;key<n;key+=256) row[key]*=inv;
}
__global__ void encoder_merge_heads_kernel(const float* packed,float* out,int n,int heads,int d) {
    int i=blockIdx.x*blockDim.x+threadIdx.x;
    if(i<n*heads*d) {int c=i%d,head=(i/d)%heads,t=i/(heads*d);out[i]=packed[(head*n+t)*d+c];}
}
extern "C" int psionic_encoder_softmax(void* scores,const void* mask,int n,int heads,int radius,void* stream) {
    encoder_masked_softmax_kernel<<<n*heads,256,0,(cudaStream_t)stream>>>((float*)scores,(const int*)mask,n,radius);
    return int(cudaGetLastError());
}
extern "C" int psionic_encoder_merge(const void* packed,void* out,int n,int heads,int d,void* stream) {
    encoder_merge_heads_kernel<<<(n*heads*d+255)/256,256,0,(cudaStream_t)stream>>>((const float*)packed,(float*)out,n,heads,d);
    return int(cudaGetLastError());
}
