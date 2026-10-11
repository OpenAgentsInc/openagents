// Preprocessor-only diagnostic. No kernel and no executable work.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
#if __TENSOR_OPS_SUPPORT_DEPLOYMENT_TARGET_26_2
#define CLEF_COORDINATE_OPERAND_AWARE 1
#else
#define CLEF_COORDINATE_OPERAND_AWARE 0
#endif
