/* Independent public ABI oracle: concrete timelines, exact audio bytes,
 * silence/refusal atomicity and a full60-second segmented capture. */
#include "dictation-face.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void *ready(void){void*s=dct_new();assert(s);assert(dct_phase(s)==0);dct_enable(s,1,0);assert(!dct_begin(s,0));dct_active(s,1);assert(!dct_begin(s,0));dct_ready(s,1);assert(dct_capture_allowed(s));return s;}
static void wave(float*out,int n){for(int i=0;i<n;i++)out[i]=(i%16<8)?0.25f:-0.25f;}
int main(void){
 float block[320];wave(block,320);void*s=ready();
 for(int i=0;i<20;i++)dct_audio(s,block,320,16000,i*20);
 uint64_t token=dct_begin(s,400);assert(token==1&&dct_samples(s)==6400);dct_release(s,700);
 for(int i=20;i<44;i++)dct_audio(s,block,320,16000,i*20);
 dct_audio(s,block,240,16000,880);dct_tick(s,895);assert(dct_phase(s)==3);
 dct_audio(s,block+240,80,16000,895);dct_tick(s,900);assert(dct_phase(s)==4&&dct_samples(s)==14400);
 dct_release(s,10000);assert(dct_phase(s)==4&&dct_samples(s)==14400);
 float*out=malloc(960000*sizeof(float));assert(out);for(int i=0;i<14400;i++)out[i]=17;
 assert(dct_copy_audio(s,0,out,14399)== -1);for(int i=0;i<14400;i++)assert(out[i]==17);
 assert(dct_copy_audio(s,0,out,14400)==14400);for(int i=0;i<14400;i++)assert(out[i]==block[i%320]);
 assert(dct_segment(s,0)==14400&&dct_segment(s,14400)==0);assert(!dct_decode_begin(s,2)&&dct_decode_begin(s,token));
 assert(dct_append_text(s,token,(const uint8_t*)"first",5)&&dct_append_text(s,token,(const uint8_t*)"second",6));
 dct_release(s,10001);assert(dct_text_count(s)==12&&dct_samples(s)==14400);uint8_t partial[32];memset(partial,17,sizeof partial);assert(dct_copy_text(s,partial,32)== -1&&partial[0]==17);
 assert(!dct_result(s,2,1)&&dct_samples(s)==14400);assert(dct_result(s,token,1)&&dct_samples(s)==0);
 uint8_t text[32];memset(text,17,sizeof text);assert(dct_copy_text(s,text,11)== -1&&text[0]==17);assert(dct_copy_text(s,text,32)==12&&!memcmp(text,"first second",12));
 dct_cancel(s);assert(dct_outcome(s)==3&&dct_text_count(s)==0);dct_active(s,0);assert(!dct_capture_allowed(s));dct_free(s);
 s=ready();uint64_t first=dct_begin(s,0);dct_release(s,299);assert(dct_outcome(s)==1&&dct_samples(s)==0);uint64_t second=dct_begin(s,400);assert(second>first);float quiet[320]={0};
 for(int i=0;i<30;i++)dct_audio(s,quiet,320,16000,400+i*20);dct_release(s,800);dct_tick(s,1000);assert(dct_outcome(s)==2&&dct_samples(s)==0);dct_free(s);
 s=ready();for(int i=0;i<25;i++)dct_audio(s,block,320,16000,i*20);assert(dct_begin(s,500));
 for(int i=25;i<3002;i++)dct_audio(s,block,320,16000,i*20);dct_audio(s,block,160,16000,60040);dct_tick(s,60050);
 assert(dct_phase(s)==4&&dct_outcome(s)==6&&dct_samples(s)==960000);int offset=0,count=0;while(offset<960000){int n=dct_segment(s,offset);assert(n>=320&&n<=327520);offset+=n;count++;}assert(offset==960000&&count==3);
 assert(dct_copy_audio(s,0,out,960000)==960000);for(int i=0;i<960000;i++)assert(out[i]==block[(i+800)%320]);
 dct_fault(s,1);assert(!dct_model_ready(s)&&!dct_capture_allowed(s)&&dct_samples(s)==0&&dct_outcome(s)==4&&dct_fault_reason(s)==1);dct_free(s);
 s=ready();token=dct_begin(s,0);for(int i=0;i<25;i++)dct_audio(s,block,320,16000,i*20);dct_release(s,300);dct_tick(s,500);assert(dct_phase(s)==4&&dct_decode_begin(s,token));assert(!dct_result(s,token,1)&&dct_outcome(s)==2&&dct_text_count(s)==0);dct_free(s);
 s=ready();assert(dct_begin(s,0));float bad=NAN;dct_audio(s,&bad,1,16000,0);assert(dct_fault_reason(s)==5&&dct_samples(s)==0&&!dct_capture_allowed(s));dct_free(s);
 free(out);puts("dictation controller ABI: exact partial tail, silence/tap, ticket/result, atomic copy and60-second3-segment cap PASS");return 0;
}
