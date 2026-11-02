// Test transport only: all format, color and byte admission is Coax.
#include "texture.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static uint32_t le32(const unsigned char *p){return (uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);}
static void put(uint32_t value){unsigned char bytes[4];for(int i=0;i<4;i++)bytes[i]=(unsigned char)(value>>(i*8));assert(fwrite(bytes,1,4,stdout)==4);}
static void number(float value){uint32_t bits;memcpy(&bits,&value,4);put(bits);}
int main(int argc,char **argv){
 assert(argc==2||argc==3);FILE *file=fopen(argv[1],"rb");assert(file);unsigned char header[8];assert(fread(header,1,8,file)==8&&!memcmp(header,"TEX1",4));uint32_t count=le32(header+4);
 for(uint32_t i=0;i<count;i++){
  unsigned char row[40];assert(fread(row,1,sizeof row,file)==sizeof row);uint32_t format=le32(row),bytes=le32(row+4);assert(format<=7&&bytes<=24);uint64_t raw=(uint64_t)le32(row+8)|((uint64_t)le32(row+12)<<32);int64_t offset;memcpy(&offset,&raw,8);
  coax_t_RenderTexelResult value=render_texture_native_texel((uint8_t)format,(coax_v_u8){row+16,bytes},offset);
  if(argc==3&&!strcmp(argv[2],"swap-rb")){float swap=value.colour.r;value.colour.r=value.colour.b;value.colour.b=swap;}
  if(argc==3&&!strcmp(argv[2],"drop-alpha"))value.colour.a=0;
  if(argc==3&&!strcmp(argv[2],"clear-status"))value.status=0;
  put(value.status);number(value.colour.r);number(value.colour.g);number(value.colour.b);number(value.colour.a);
 }
 assert(fgetc(file)==EOF);assert(!fclose(file));return 0;
}
