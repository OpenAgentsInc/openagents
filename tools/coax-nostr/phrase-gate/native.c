/* Public-fixture transport and sanitizer observations only, no gate policy. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
extern uintptr_t pg_worker_password(void),pg_worker_record(void),pg_worker_message(void),pg_worker_relay(void),pg_worker_recipient(void),pg_worker_entropy(void),pg_worker_output(void);
extern uint32_t pg_worker_preflight(int32_t,int32_t,int32_t,int32_t,int32_t,uint32_t),pg_worker_begin(int32_t,int32_t,int32_t,int32_t,int32_t,uint32_t),pg_worker_advance(int32_t),pg_worker_dispose(void),pg_worker_clean(void),pg_worker_cancel(void),pg_worker_abort(void),pg_worker_emit(void),pg_worker_running(void),pg_worker_error(void);
extern int32_t pg_worker_written(void),pg_worker_completed(void);extern void pg_worker_test_fault(uint32_t);
static uint8_t values[5][65537],first[5][65537],expected[6145];static uint32_t sizes[5],first_sizes[5];
static uint32_t le(const uint8_t*p){return(uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;}
static uint32_t begin(uint32_t operation){
 uintptr_t addresses[]={pg_worker_password(),pg_worker_record(),pg_worker_message(),pg_worker_relay(),pg_worker_recipient()};
 for(unsigned i=0;i<5;i++)memcpy((void*)addresses[i],values[i],sizes[i]);uint8_t entropy[28];for(unsigned i=0;i<28;i++)entropy[i]=(uint8_t)i;memcpy((void*)pg_worker_entropy(),entropy,28);
 return pg_worker_begin(sizes[0],sizes[1],sizes[2],sizes[3],sizes[4],operation);
}
static void reset(void){assert(pg_worker_dispose()==0&&pg_worker_clean()==1);}
static void prepare_first(void){reset();for(unsigned i=0;i<5;i++){sizes[i]=first_sizes[i];memcpy(values[i],first[i],sizes[i]);}assert(begin(0)==0);}
int main(void){
 uint8_t header[32];unsigned records=0;
 while(fread(header,1,32,stdin)==32){
  uint32_t operation=le(header),error=le(header+4),n=le(header+28);assert(n<=6144);
  for(unsigned i=0;i<5;i++){sizes[i]=le(header+8+4*i);assert(sizes[i]<=65536);assert(fread(values[i],1,sizes[i],stdin)==sizes[i]);}
  assert(fread(expected,1,n,stdin)==n);if(!records)for(unsigned i=0;i<5;i++){first_sizes[i]=sizes[i];memcpy(first[i],values[i],sizes[i]);}
  reset();uint32_t result=pg_worker_preflight(sizes[0],sizes[1],sizes[2],sizes[3],sizes[4],operation);
  if(!result){result=begin(operation);if(!result){assert(!pg_worker_output()&&!pg_worker_written());unsigned steps=0;int32_t previous=0;while(pg_worker_running()){result=pg_worker_advance(1024);int32_t completed=pg_worker_completed();assert(completed>=previous&&completed-previous<=1024&&completed<=210000);previous=completed;assert(++steps<210002);if(result)break;}if(!result)result=pg_worker_error();}}
  assert(result==error);
  if(error){assert(!pg_worker_emit()&&!pg_worker_output()&&!pg_worker_written());}
  else {assert(pg_worker_emit()&&pg_worker_written()==(int32_t)n&&memcmp((void*)pg_worker_output(),expected,n)==0&&pg_worker_completed()==210000);}
  reset();records++;
 }
 assert(records);prepare_first();assert(pg_worker_advance(256)==0&&pg_worker_completed()==256);assert(pg_worker_cancel()==12&&!pg_worker_output());reset();
 prepare_first();assert(pg_worker_advance(256)==0);assert(pg_worker_abort()==11&&!pg_worker_output());reset();
 prepare_first();pg_worker_test_fault(1);uint32_t result=0;while(pg_worker_running())result=pg_worker_advance(1024);assert(result==13&&!pg_worker_output()&&!pg_worker_emit());assert(pg_worker_dispose()==13);pg_worker_test_fault(0);reset();
 for(unsigned i=0;i<3;i++){int32_t budgets[]={0,-1,1025};prepare_first();assert(pg_worker_advance(budgets[i])&&!pg_worker_output());reset();}
 printf("%u native ASan/UBSan full-round/admission records, cancellation/abort/budget/cleanup failures and identified-buffer observations passed\n",records);return 0;
}
