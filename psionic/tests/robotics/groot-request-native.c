/* Firm reason2: independent request grammar, scalar positions and canary oracle.
   Optional original-tokenizer DATA is private, never executed code. */
#include "groot-request.h"
#include <assert.h>
#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned cases;
static coax_v_u8 bytes(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}
static coax_v_s64 ints(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_string text(const char *p,int64_t n){return(coax_string){p,n};}
static int append(char *p,int at,const char *s){int n=(int)strlen(s);memcpy(p+at,s,(size_t)n);return at+n;}
static int render(const char *instruction,int n,char *out){
  int at=append(out,0,"<|im_start|>user\n");
  for(int camera=0;camera<2;camera++){
    at=append(out,at,"<|vision_start|>");for(int patch=0;patch<64;patch++)at=append(out,at,"<|image_pad|>");at=append(out,at,"<|vision_end|>");
  }
  for(int i=0;i<n;i++){unsigned char c=(unsigned char)instruction[i];assert(c<128);if(isalnum(c)||c=='_'||isspace(c)||(c>=28&&c<=31))out[at++]=(char)tolower(c);}
  return append(out,at,"<|im_end|>\n");
}
static void tokens(int64_t *p,int n){
  for(int i=0;i<n;i++)p[i]=1;
  p[0]=151644;p[1]=872;p[2]=198;p[3]=151652;
  for(int i=4;i<68;i++)p[i]=151655;p[68]=151653;p[69]=151652;
  for(int i=70;i<134;i++)p[i]=151655;p[134]=151653;p[n-2]=151645;p[n-1]=198;
}
static void positions(int64_t out[3][512],int n){
  int cursor=0,next=0;
  for(int i=0;i<4;i++,cursor++,next++)for(int axis=0;axis<3;axis++)out[axis][cursor]=next;
  for(int camera=0;camera<2;camera++){
    for(int row=0;row<8;row++)for(int col=0;col<8;col++,cursor++){
      out[0][cursor]=next;out[1][cursor]=next+row;out[2][cursor]=next+col;
    }
    next+=8;
    if(camera==0)for(int i=0;i<2;i++,cursor++,next++)for(int axis=0;axis<3;axis++)out[axis][cursor]=next;
  }
  while(cursor<n){for(int axis=0;axis<3;axis++)out[axis][cursor]=next;cursor++;next++;}
}
static void sentinels(int64_t *p){for(int i=0;i<3073;i++)p[i]=-7;}
static void untouched(const int64_t *p){for(int i=0;i<3073;i++)assert(p[i]==-7);}
int main(int argc,char **argv){
  assert(argc<=2);uint8_t output[8193],work[8193];char expected[8192],instruction[4097];
  for(int c=0;c<256;c++){
    memset(output,7,sizeof output);char one=(char)c;coax_t_GrRead r=gr_render(text(&one,1),bytes(output,8193),bytes(work,8193),0);
    if(c>=128){assert(r.status==2&&r.written==0);for(unsigned i=0;i<sizeof output;i++)assert(output[i]==7);}
    else {int n=render(&one,1,expected);assert(r.status==0&&r.written==n&&!memcmp(output,expected,(size_t)n));for(unsigned i=(unsigned)n;i<sizeof output;i++)assert(output[i]==7);}
    cases++;
  }
  memset(instruction,'A',4097);
  for(int n=-1;n<=4097;n++){
    memset(output,7,sizeof output);coax_t_GrRead r=gr_render(text(instruction,n),bytes(output,8193),bytes(work,8193),0);
    if(n<0||n>4096){assert(r.status==1&&r.written==0);for(unsigned i=0;i<sizeof output;i++)assert(output[i]==7);}
    else {int wanted=render(instruction,n,expected);assert(r.status==0&&r.written==wanted&&!memcmp(output,expected,(size_t)wanted));assert(output[wanted]==7);}cases++;
  }
  for(int which=0;which<3;which++){
    memset(output,7,sizeof output);int n=render("open the drawer",15,expected);
    coax_t_GrRead r=gr_render(text("open the drawer",15),bytes(output,which==0?n-1:n),bytes(work,which==1?n-1:n),which==2);
    assert(r.status==(which==2?4:3)&&r.written==0);for(unsigned i=0;i<sizeof output;i++)assert(output[i]==7);cases++;
  }
  int64_t ids[513],out[3073],pos[3][512];
  for(int n=137;n<=512;n++){
    tokens(ids,n);positions(pos,n);
    int pads[]={0,n,512};
    for(int choice=0;choice<3;choice++){
      int count=pads[choice]?pads[choice]:n,left=count-n;sentinels(out);
      coax_t_GrRead r=gr_plan(ints(ids,n),pads[choice],ints(out,6*count),0);
      assert(r.status==0&&r.written==6*count&&r.content==n&&r.count==count&&r.rope_delta==n-112-count);
      for(int i=0;i<count;i++){
        if(i<left){assert(out[i]==151643&&out[count+i]==0&&out[2*count+i]==0);for(int axis=0;axis<3;axis++)assert(out[(3+axis)*count+i]==1);}
        else {int source=i-left;assert(out[i]==ids[source]&&out[count+i]==1&&out[2*count+i]==(ids[source]==151655));for(int axis=0;axis<3;axis++)assert(out[(3+axis)*count+i]==pos[axis][source]);}
        cases++;
      }
      for(int i=6*count;i<3073;i++)assert(out[i]==-7);
    }
  }
  tokens(ids,140);
  for(int i=0;i<140;i++){
    int64_t before=ids[i];ids[i]=-1;sentinels(out);coax_t_GrRead r=gr_plan(ints(ids,140),0,ints(out,3073),0);assert(r.status==5&&r.written==0);untouched(out);ids[i]=before;cases++;
    if(i<135||i>=138){ids[i]=before+1;sentinels(out);r=gr_plan(ints(ids,140),0,ints(out,3073),0);assert(r.status==5&&r.written==0);untouched(out);ids[i]=before;cases++;}
  }
  for(int which=0;which<6;which++){
    sentinels(out);int n=which==0?136:which==1?513:140,pad=which==2?-1:which==3?139:0,capacity=which==4?839:3073;
    coax_t_GrRead r=gr_plan(ints(ids,n),pad,ints(out,capacity),which==5);assert(r.status==(which<4?1:which==4?3:4)&&r.written==0);untouched(out);cases++;
  }
  if(argc==2){
    FILE *f=fopen(argv[1],"rb");assert(f);assert(!fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0);rewind(f);uint8_t *table=malloc((size_t)size);assert(table&&fread(table,1,(size_t)size,f)==(size_t)size);assert(!fclose(f));
    int n=render("open the drawer",15,expected);int64_t encoded[2048],stage[2048],scalars[8192],symbols[2048];uint8_t normal[8192];
    coax_t_PtResult e=pt_encode_checked(bytes(table,size),text(expected,n),ints(encoded,2048),ints(stage,2048),bytes(normal,8192),ints(scalars,8192),ints(symbols,2048),0);assert(e.status==0&&e.written==140);
    tokens(ids,140);ids[135]=2508;ids[136]=279;ids[137]=26482;for(int i=0;i<140;i++){assert(encoded[i]==ids[i]);cases++;}free(table);
  }
  printf("GR00T request: %u independent native cases passed%s\n",cases,argc==2?" with private original tokenizer":"");return 0;
}
