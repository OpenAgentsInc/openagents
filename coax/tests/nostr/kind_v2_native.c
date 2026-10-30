#include "kind.h"
#include <stdint.h>
static uint32_t decode(uint8_t *data,int64_t n,uint32_t op) {
    coax_t_KindInput input={0};input.wire.data=data;input.wire.count=n;input.op=op;return kind_decode(input);
}
int run_kinds(void) {
    uint8_t zero[]={24,0}, max[]={24,255,255,3}, bad[]={24,128,128,4};
    uint8_t list[]={10,3,255,255,3}, invalid[]={10,3,128,128,4};
    uint8_t nested[]={34,5,10,3,128,128,4}, nonminimal[]={24,129,0};
    if(decode(zero,sizeof zero,0)||decode(max,sizeof max,0)||decode(max,sizeof max,1))return 1;
    if(decode(bad,sizeof bad,0)!=7 || decode(bad,sizeof bad,1)!=7) return 2;
    if(decode(list,sizeof list,2)||decode(invalid,sizeof invalid,2)!=7) return 3;
    if(decode(nested,sizeof nested,3)!=7) return 4;
    if(decode(nonminimal,sizeof nonminimal,0) || decode(nonminimal,sizeof nonminimal,5)!=1) return 5;
    uint8_t event[160]={0};event[0]=10;event[1]=32;event[34]=18;event[35]=32;event[68]=24;event[69]=1;event[70]=32;
    event[71]=128;event[72]=128;event[73]=4;event[74]=50;event[75]=1;event[76]=120;event[77]=58;event[78]=64;
    if(decode(event,143,4)!=7) return 6;
    event[71]=255;event[72]=255;event[73]=3;
    if(decode(event,143,4)!=0) return 7;
    return 0;
}
#ifndef __wasm__
int main(void) { return run_kinds(); }
#endif
