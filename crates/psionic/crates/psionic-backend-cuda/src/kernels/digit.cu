// Float32 convolution, ReLU, and floor max pooling for cropped digit images.
#include <cuda_runtime.h>
#include <math_constants.h>

__global__ void psionic_digit_conv_pool_kernel(const float* input,const float* weights,const float* bias,float* out,int channels,int width,int outputs,int pool) {
    int next=width/pool;
    int index=blockIdx.x*blockDim.x+threadIdx.x;
    if(index>=outputs*next*next) return;
    int oc=index/(next*next),y=(index/next)%next,x=index%next;
    float best=-CUDART_INF_F;
    for(int py=0;py<pool;++py) for(int px=0;px<pool;++px) {
        float sum=0;
        for(int c=0;c<channels;++c) for(int ky=0;ky<5;++ky) for(int kx=0;kx<5;++kx) {
            int iy=y*pool+py+ky-2,ix=x*pool+px+kx-2;
            if(iy>=0 && ix>=0 && iy<width && ix<width) sum+=input[(c*width+iy)*width+ix]*weights[((oc*channels+c)*5+ky)*5+kx];
        }
        best=fmaxf(best,sum+bias[oc]);
    }
    out[index]=fmaxf(0.0f,best);
}
__global__ void psionic_digit_head_kernel(const float* input,const float* weights,const float* bias,float* out) {
    int digit=threadIdx.x;
    if(digit<10) {float sum=0;for(int i=0;i<256;++i) sum+=input[i]*weights[i*10+digit];out[digit]=sum+bias[digit];}
}
extern "C" int psionic_digit_conv_pool(const void* input,const void* weights,const void* bias,void* out,int channels,int width,int outputs,int pool,void* stream) {
    int count=outputs*(width/pool)*(width/pool);
    psionic_digit_conv_pool_kernel<<<(count+127)/128,128,0,(cudaStream_t)stream>>>((const float*)input,(const float*)weights,(const float*)bias,(float*)out,channels,width,outputs,pool);
    return int(cudaGetLastError());
}
extern "C" int psionic_digit_head(const void* input,const void* weights,const void* bias,void* out,void* stream) {
    psionic_digit_head_kernel<<<1,32,0,(cudaStream_t)stream>>>((const float*)input,(const float*)weights,(const float*)bias,(float*)out);
    return int(cudaGetLastError());
}
