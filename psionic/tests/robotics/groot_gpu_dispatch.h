/* Shared effects-only dispatcher. Include the Coax-generated graph header
   first; it supplies GvpNode/GvpArg. All order/shape/argument choices are Coax. */
#ifndef PSIONIC_GROOT_GPU_DISPATCH_H
#define PSIONIC_GROOT_GPU_DISPATCH_H
#include "metal-host.h"
#include <stddef.h>
static PmStatus groot_gpu_dispatch(PmBatch batch,coax_t_GvpNode node,const PmKernel *kernels,size_t kernel_count,const PmBuffer *buffers,size_t buffer_count){
 if(node.status||node.argc<0||node.argc>16||node.kernel<0||(size_t)node.kernel>=kernel_count)return PM_ARGUMENT;
 PmArg args[16]={0};
 for(int i=0;i<node.argc;i++){coax_t_GvpArg a=node.args.e[i];args[i].kind=(PmArgKind)a.kind;if(a.kind==PM_ARG_INT)args[i].i=a.integer;else if(a.kind==PM_ARG_F32)args[i].f=a.real;else if(a.kind==PM_ARG_VIEW){if(a.buffer<0||(size_t)a.buffer>=buffer_count)return PM_ARGUMENT;args[i].buffer=buffers[a.buffer];args[i].offset=a.offset;args[i].count=a.count;}else return PM_ARGUMENT;}
 return pm_launch(batch,kernels[node.kernel],node.items,(int)node.lanes,(int)node.block,args,(int)node.argc);
}
#endif
