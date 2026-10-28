#include "QR_SOURCE"
#define EX(name) __attribute__((export_name(name)))
static uint8_t input[413],output[5930];static coax_t_QrResult result;
EX("input") uintptr_t qr_input(void){return(uintptr_t)input;}
EX("output") uintptr_t qr_output(void){return(uintptr_t)output;}
EX("encode") int32_t encode(int32_t n,int32_t capacity,int32_t mask){result=qr_encode_mask((coax_v_u8){input,n},(coax_v_u8){output,capacity},mask);return(int32_t)result.status;}
EX("size") int32_t size(void){return(int32_t)result.size;}
EX("mask") int32_t mask(void){return(int32_t)result.mask;}
EX("penalty") int32_t penalty(void){return(int32_t)result.penalty;}
