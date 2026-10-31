#include "panda-query-source.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
static size_t checks;
#define NEED(x) do{checks++;if(!(x)){fprintf(stderr,"query source native line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PcSample)==1152,"existing sample ABI");
static void read_bytes(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static coax_t_PqsResult call(int kind,uint64_t*w,int64_t nw,uint64_t*q,int64_t nq,uint64_t*v,int64_t nv,int se,int re,coax_t_PcSample*out,int64_t no){return pqs_select(kind,(coax_v_u64){w,nw},(coax_v_u64){q,nq},(coax_v_u64){v,nv},se,re,(coax_v_t_PcSample){out,no});}
static void refused(int kind,uint64_t*w,int64_t nw,uint64_t*q,int64_t nq,uint64_t*v,int64_t nv,int se,int re,int64_t no,int code,int detail){coax_t_PcSample out[2],old[2];memset(out,0xa5,sizeof out);memcpy(old,out,sizeof old);coax_t_PqsResult r=call(kind,w,nw,q,nq,v,nv,se,re,out,no);NEED(r.code==code&&r.detail==detail&&!memcmp(out,old,sizeof out));}
int main(int argc,char**argv){
 if(argc!=3)return 2;FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx");NEED(input&&output);uint32_t h[2];read_bytes(input,h,sizeof h);NEED(h[0]==UINT32_C(0x31535150)&&h[1]==15);uint64_t first[173],fq[9],fv[9];
 for(int row=0;row<15;row++){uint64_t w[173],q[9],v[9];read_bytes(input,w,sizeof w);read_bytes(input,q,sizeof q);read_bytes(input,v,sizeof v);if(row==0){memcpy(first,w,sizeof w);memcpy(fq,q,sizeof q);memcpy(fv,v,sizeof v);}for(int kind=0;kind<2;kind++){coax_t_PcSample out[2],old;memset(out,0xa5,sizeof out);memcpy(&old,&out[1],sizeof old);coax_t_PqsResult r=call(kind,w,173,q,9,v,9,0,0,out,1);NEED(r.code==0&&r.detail==0&&!memcmp(&out[1],&old,sizeof old));double values[144];memcpy(values,out,sizeof values);for(int i=0;i<144;i++)NEED(isfinite(values[i]));put(output,out,sizeof out[0]);}}
 NEED(fgetc(input)==EOF);uint64_t w[174]={0},q[10]={0},v[10]={0};memcpy(w,first,sizeof first);memcpy(q,fq,sizeof fq);memcpy(v,fv,sizeof fv);
 int64_t sizes[4]={173,9,9,1};for(int field=0;field<4;field++){int64_t old=sizes[field],wrong[]={-1,old-1,old+1};for(int k=0;k<3;k++){sizes[field]=wrong[k];refused(8,w,sizes[0],q,sizes[1],v,sizes[2],1,1,sizes[3],COAX_PqsCode_shape,COAX_PcStatus_ok);}sizes[field]=old;}
 refused(-1,w,173,q,9,v,9,1,1,1,COAX_PqsCode_kind,0);refused(2,w,173,q,9,v,9,1,1,1,COAX_PqsCode_kind,0);for(int kind=0;kind<2;kind++){refused(kind,w,173,q,9,v,9,1,0,1,COAX_PqsCode_effect,0);refused(kind,w,173,q,9,v,9,0,1,1,COAX_PqsCode_effect,0);}
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000001)};
 for(int kind=0;kind<2;kind++)for(int role=0;role<3;role++){uint64_t*p=role==0?w:role==1?q:v;int count=role==0?173:9;for(int i=0;i<count;i++){uint64_t old=p[i];for(int k=0;k<3;k++){p[i]=bad[k];refused(kind,w,173,q,9,v,9,0,0,1,COAX_PqsCode_number,COAX_PcStatus_number);}p[i]=old;}}
 double large=101;memcpy(q,&large,8);refused(1,w,173,q,9,v,9,0,0,1,COAX_PqsCode_sample,COAX_PcStatus_range);NEED(!fclose(input)&&!fclose(output));printf("{\"passed\":true,\"cases\":15,\"sources\":2,\"output_words\":4320,\"assertions\":%zu}\n",checks);return 0;
}
