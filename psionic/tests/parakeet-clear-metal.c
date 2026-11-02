/* Firm reason: 2 (independent shared-buffer clear effect oracle).
 * Inject tiny shared-memory buffers; execute no GPU or model arithmetic. */
#include "../platform/parakeet-metal.c"
#include <assert.h>
static void filled(const void*p,size_t n,unsigned char v){const unsigned char*b=p;for(size_t i=0;i<n;i++)assert(b[i]==v);}
int main(void){
 unsigned char memory[4][104];PmBuffer old[4];
 for(int i=0;i<4;i++){memset(memory[i],0x31+i,sizeof memory[i]);bufs[i]=(PmBuffer)(memory[i]+1);old[i]=bufs[i];bases[i]=(float*)(memory[i]+1);sizes[i]=101;}
 ready=1;loaded=(const float*)(memory[0]+1);loaded_count=17;seconds=1.25;memset(error_text,0x77,sizeof error_text);
 PmModule saved_module=(PmModule)(uintptr_t)0x100;mod=saved_module;PmKernel saved_kernel=(PmKernel)(uintptr_t)0x200;kernels[0]=saved_kernel;
 pk_metal_clear_work();filled(memory[0],sizeof memory[0],0x31);
 for(int i=1;i<4;i++){filled(memory[i]+1,101,0);assert(memory[i][0]==0x31+i&&memory[i][102]==0x31+i&&memory[i][103]==0x31+i);}
 for(int i=0;i<4;i++)assert(bufs[i]==old[i]&&bases[i]==(float*)(memory[i]+1)&&sizes[i]==101);
 assert(ready&&loaded==(const float*)(memory[0]+1)&&loaded_count==17&&mod==saved_module&&kernels[0]==saved_kernel&&seconds==0);filled(error_text,sizeof error_text,0);
 pk_metal_clear_work();for(int i=0;i<4;i++){bufs[i]=0;bases[i]=0;sizes[i]=0;}mod=0;kernels[0]=0;ready=0;loaded=0;loaded_count=0;
 puts("Parakeet Metal privacy clear: full work capacities zero, weight buffer and resident bindings retained");return 0;
}
