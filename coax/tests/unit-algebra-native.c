/* Tooling transport only. Every result comes from the exported Coax producer. */
#include "unit-algebra.h"
#include <stdio.h>
#include <string.h>
#include <inttypes.h>

static uint64_t get64(const unsigned char *p) {
  uint64_t x=0;for(unsigned i=0;i<8;i++) x|=(uint64_t)p[i]<<(8*i);return x;
}
static int64_t signed64(const unsigned char *p) {
  uint64_t x=get64(p);int64_t y;memcpy(&y,&x,sizeof(y));return y;
}
static void put64(unsigned char *p,int64_t value) {
  uint64_t x;memcpy(&x,&value,sizeof(x));for(unsigned i=0;i<8;i++) p[i]=(unsigned char)(x>>(8*i));
}
static coax_t_Q_Unit unit(const unsigned char *p) {
  return (coax_t_Q_Unit){signed64(p),signed64(p+8),signed64(p+16),signed64(p+24),signed64(p+32),signed64(p+40)};
}
int main(int argc,char **argv) {
  if(argc!=3) return 2;
  FILE *in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");if(!in||!out) return 3;
  unsigned char header[8];if(fread(header,1,8,in)!=8) return 4;
  uint64_t count=get64(header);if(count>1000000) return 5;
  for(uint64_t i=0;i<count;i++) {
    unsigned char row[104],result[56];if(fread(row,1,sizeof(row),in)!=sizeof(row)) return 6;
    coax_t_Q_Unit a=unit(row+8),b=unit(row+56);coax_t_UA_Result r;
    switch(get64(row)) {
      case 0:r=UA_normalize(a);break;
      case 1:r=UA_product(a,b);break;
      case 2:r=UA_quotient(a,b);break;
      case 3:r=UA_ratio(a,b);break;
      default:return 7;
    }
    int64_t fields[7]={r.status,r.unit.mass,r.unit.length,r.unit.time,r.unit.angle,r.unit.numerator,r.unit.denominator};
    for(unsigned j=0;j<7;j++) put64(result+8*j,fields[j]);
    if(fwrite(result,1,sizeof(result),out)!=sizeof(result)) return 8;
  }
  if(fgetc(in)!=EOF||ferror(in)||fclose(in)) return 9;
  if(fclose(out)) return 10;
  printf("%" PRIu64 " exact unit metadata results\n",count);return 0;
}
