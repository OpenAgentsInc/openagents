#include "range.h"
#include <stdint.h>
#include <string.h>
static int64_t write_value(uint32_t op, uint32_t value, uint8_t *out, int64_t n) {
    coax_t_RangeBuffer o = {0}; o.bytes.data=out; o.bytes.count=n;
    return range_write(op,value,o);
}
static uint32_t read_value(uint8_t *data, int64_t n, uint32_t op) {
    coax_t_RangeBuffer i = {0}; i.bytes.data=data; i.bytes.count=n;
    return range_read(i,op);
}
int run_ranges(void) {
    uint8_t out[64]={0};
    if (write_value(0,0,out,sizeof out)!=2 || out[0]!=8 || out[1]!=0) return 1;
    if (write_value(0,1,out,sizeof out)!=-7) return 2;
    if (write_value(1,0,out,sizeof out)!=2 || out[0]!=16 || out[1]!=0) return 3;
    if (write_value(1,65535,out,sizeof out)!=4 || memcmp(out,(uint8_t[]){16,255,255,3},4)) return 4;
    if (write_value(1,65536,out,sizeof out)!=-7) return 5;
    if (write_value(1,UINT32_MAX,out,sizeof out)!=-7) return 6;
    if (write_value(2,65535,out,sizeof out)!=4 || memcmp(out,(uint8_t[]){24,255,255,3},4)) return 7;
    if (write_value(2,65536,out,sizeof out)!=-7) return 8;
    if (write_value(3,UINT32_MAX,out,sizeof out)!=6 || memcmp(out,(uint8_t[]){32,255,255,255,255,15},6)) return 9;
    if (write_value(1,65536,out,1)!=-2) return 10; // output capacity precedes final range validation
    uint8_t mixed[]={24,0,26,3,255,255,3,24,1};
    uint8_t badfirst[]={26,4,128,128,4,0};
    uint8_t badmiddle[]={26,5,0,128,128,4,0};
    uint8_t badlast[]={26,4,0,128,128,4};
    uint8_t nonminimal[]={16,129,0};
    uint8_t negative[]={16,255,255,255,255,255,255,255,255,255,1};
    uint8_t nested[]={10,4,16,128,128,4};
    if (read_value(mixed,sizeof mixed,0)!=0) return 11;
    if (read_value(badfirst,sizeof badfirst,0)!=7 || read_value(badmiddle,sizeof badmiddle,0)!=7 || read_value(badlast,sizeof badlast,0)!=7) return 12;
    if (read_value(nonminimal,sizeof nonminimal,0)!=0 || read_value(nonminimal,sizeof nonminimal,1)!=1) return 13;
    if (read_value(negative,sizeof negative,0)!=7 || read_value(nested,sizeof nested,2)!=7) return 14;
    return 0;
}
#ifndef __wasm__
int main(void) { return run_ranges(); }
#endif
