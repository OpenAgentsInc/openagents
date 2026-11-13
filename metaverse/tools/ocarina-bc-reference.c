/* Firm reason: 3 (runner for unchanged upstream bcdec reference decoder, 80859ed3b7afb1c527a2a99d70c61457bea72d0c). */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#define BCDEC_IMPLEMENTATION
#include "third_party/bcdec.h"
int main(int argc,char **argv){
 if(argc!=3)return 2;FILE *f=fopen(argv[1],"rb");if(!f)return 3;uint32_t h[4];if(fread(h,4,4,f)!=4||h[0]!=0x31584342||!h[2]||!h[3]||h[2]>4096||h[3]>4096)return 4;
 unsigned fmt=h[1],w=h[2],hh=h[3],bytes=fmt==0x1a||fmt==0x1d?8:16;if(fmt!=0x1a&&fmt!=0x1c&&fmt!=0x1d&&fmt!=0x1e&&fmt!=0x20)return 5;
 unsigned char *rgba=calloc((size_t)w*hh,4),block[16],decoded[64];if(!rgba)return 6;
 for(unsigned y=0;y<hh;y+=4)for(unsigned x=0;x<w;x+=4){if(fread(block,1,bytes,f)!=bytes)return 7;
  if(fmt==0x1a)bcdec_bc1(block,decoded,16);else if(fmt==0x1c)bcdec_bc3(block,decoded,16);else if(fmt==0x1d)bcdec_bc4(block,decoded,4);else if(fmt==0x1e)bcdec_bc5(block,decoded,8);else bcdec_bc7(block,decoded,16);
  for(unsigned yy=0;yy<4&&y+yy<hh;yy++)for(unsigned xx=0;xx<4&&x+xx<w;xx++){unsigned o=((y+yy)*w+x+xx)*4,i=(yy*4+xx)*(fmt==0x1d?1:fmt==0x1e?2:4);for(unsigned c=0;c<4;c++)rgba[o+c]=fmt==0x1d?(c==0?decoded[i]:c==3?255:0):fmt==0x1e?(c<2?decoded[i+c]:c==3?255:0):decoded[i+c];}
 }
 if(fgetc(f)!=EOF)return 8;fclose(f);f=fopen(argv[2],"wb");if(!f)return 9;size_t n=(size_t)w*hh*4;int ok=fwrite(rgba,1,n,f)==n&&fclose(f)==0;free(rgba);return ok?0:10;
}
