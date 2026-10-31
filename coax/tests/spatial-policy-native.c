/* Logical-value transport for the independent spatial-policy table oracle. */
#include "spatial-policy.h"
#include <stdio.h>
#include <string.h>
#include <inttypes.h>
static uint64_t get64(const unsigned char*p){uint64_t x=0;for(unsigned i=0;i<8;i++)x|=(uint64_t)p[i]<<(8*i);return x;}
static int64_t s64(const unsigned char*p){uint64_t x=get64(p);int64_t y;memcpy(&y,&x,8);return y;}
static void put64(unsigned char*p,int64_t x){uint64_t y;memcpy(&y,&x,8);for(unsigned i=0;i<8;i++)p[i]=(unsigned char)(y>>(8*i));}
static coax_t_SP_Type type(const unsigned char*p){return(coax_t_SP_Type){(coax_t_SP_Kind)s64(p),s64(p+8),s64(p+16),s64(p+24)};}
int main(int argc,char**argv){
 if(argc!=3)return 2;FILE*in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");if(!in||!out)return 3;
 unsigned char header[8];if(fread(header,1,8,in)!=8)return 4;uint64_t count=get64(header);if(count>1000000)return 5;
 for(uint64_t i=0;i<count;i++){
  unsigned char row[160],output[48];if(fread(row,1,sizeof(row),in)!=sizeof(row))return 6;
  coax_t_SP_Frame f={s64(row+16),(coax_t_SP_Convention)s64(row+24),(coax_t_SP_Continuity)s64(row+32),s64(row+40)};
  coax_t_SP_Type a=type(row+48),b=type(row+80);coax_t_Q_Unit u={s64(row+112),s64(row+120),s64(row+128),s64(row+136),s64(row+144),s64(row+152)};
  coax_t_SP_Result r={0};int64_t flag=0;
  switch(get64(row)){
   case 0:flag=SP_frame_valid(f);break;
   case 1:flag=SP_type_valid(a);break;
   case 2:r.status=SP_admit(a,f,u);break;
   case 3:r=SP_binary((coax_t_SP_Op)s64(row+8),a,b);break;
   case 4:r=SP_negate(a);break;
   default:return 7;
  }
  int64_t fields[6]={r.status,r.result.kind,r.result.frame,r.result.unit,r.result.rep,flag};
  for(unsigned j=0;j<6;j++)put64(output+8*j,fields[j]);if(fwrite(output,1,sizeof(output),out)!=sizeof(output))return 8;
 }
 if(fgetc(in)!=EOF||ferror(in)||fclose(in))return 9;if(fclose(out))return 10;
 printf("%" PRIu64 " exact spatial metadata decisions\n",count);return 0;
}
