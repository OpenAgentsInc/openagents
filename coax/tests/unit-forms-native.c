/* Independent-test transport. Nominal unit arithmetic remains in Coax. */
#include "unit-forms.h"
#include <stdio.h>
#include <string.h>
#include <inttypes.h>
static uint64_t get64(const unsigned char *p){uint64_t x=0;for(unsigned i=0;i<8;i++)x|=(uint64_t)p[i]<<(8*i);return x;}
static int64_t signed64(const unsigned char *p){uint64_t x=get64(p);int64_t y;memcpy(&y,&x,8);return y;}
static void put64(unsigned char *p,int64_t v){uint64_t x;memcpy(&x,&v,8);for(unsigned i=0;i<8;i++)p[i]=(unsigned char)(x>>(8*i));}
static coax_t_UF_Form form(const unsigned char *p){coax_t_UF_Form f={0};f.count=signed64(p);for(unsigned i=0;i<8;i++){f.atoms.e[i]=signed64(p+8+8*i);f.powers.e[i]=signed64(p+72+8*i);}return f;}
int main(int argc,char **argv){
  if(argc!=3)return 2;FILE *in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");if(!in||!out)return 3;
  unsigned char header[8];if(fread(header,1,8,in)!=8)return 4;uint64_t count=get64(header);if(count>1000000)return 5;
  for(uint64_t i=0;i<count;i++){
    unsigned char input[280],output[152];if(fread(input,1,sizeof(input),in)!=sizeof(input))return 6;
    coax_t_UF_Form a=form(input+8),b=form(input+144);coax_t_UF_Result r={0};int64_t flag=0;
    switch(get64(input)){
      case 0:r.form=UF_one();break;
      case 1:r=UF_atom(a.atoms.e[0]);break;
      case 2:flag=UF_valid(a);break;
      case 3:flag=UF_equal(a,b);break;
      case 4:r=UF_product(a,b);break;
      case 5:r=UF_quotient(a,b);break;
      default:return 7;
    }
    put64(output,r.status);put64(output+8,r.form.count);
    for(unsigned j=0;j<8;j++){put64(output+16+8*j,r.form.atoms.e[j]);put64(output+80+8*j,r.form.powers.e[j]);}
    put64(output+144,flag);if(fwrite(output,1,sizeof(output),out)!=sizeof(output))return 8;
  }
  if(fgetc(in)!=EOF||ferror(in)||fclose(in))return 9;if(fclose(out))return 10;
  printf("%" PRIu64 " exact nominal form results\n",count);return 0;
}
