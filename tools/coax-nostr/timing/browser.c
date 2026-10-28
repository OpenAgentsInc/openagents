/* Wasm acquisition storage and callback dispatch only; Coax owns statistics. */
#include "timing.c"
#define EX(name) __attribute__((export_name(name)))
#define COUNT 1000000
static uint64_t inputs[COUNT],table[1048576];
static uint8_t classes[COUNT],raw[12+COUNT*9];
static double elapsed[COUNT];
typedef uint64_t(*fixture)(uint64_t,coax_v_u64);
static fixture volatile selected;
static volatile uint64_t observed;
static coax_t_CtResult result;
static uint64_t seed=20261028;
static uint64_t random_word(void){seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return seed;}
EX("count") int count(void){return COUNT;}
EX("times") uintptr_t times(void){return(uintptr_t)elapsed;}
EX("raw") uintptr_t raw_pointer(void){return(uintptr_t)raw;}
EX("raw_size") int raw_size(void){return sizeof(raw);}
EX("prepare") void prepare(int which){
 seed=20261028;
 for(unsigned i=0;i<COUNT;i+=2){classes[i]=(uint8_t)(random_word()&1);classes[i+1]=1-classes[i];}
 for(unsigned i=0;i<COUNT;i++)inputs[i]=classes[i]?random_word():0;
 table[0]=0;for(unsigned i=1;i<1048576;i++)table[i]=1+random_word()%1048575;
 selected=which==0?ct_fixed:which==1?ct_branch:ct_index;
}
EX("sample") void sample(int index){
 uint64_t value=0;coax_v_u64 span={table,1048576};
 for(unsigned j=0;j<8;j++)value^=selected(inputs[index],span);
 observed=value;
}
EX("finish") int finish(void){
 coax_t_CtStats stats={0};
 raw[0]='C';raw[1]='T';raw[2]='0';raw[3]='1';
 uint64_t n=COUNT;for(unsigned j=0;j<8;j++)raw[4+j]=(uint8_t)(n>>(8*j));
 for(unsigned i=0;i<COUNT;i++){
  if(!ct_observe((coax_v_t_CtStats){&stats,1},classes[i],elapsed[i]))return -1;
  raw[12+i*9]=classes[i];uint64_t v=(uint64_t)elapsed[i];
  for(unsigned j=0;j<8;j++)raw[13+i*9+j]=(uint8_t)(v>>(8*j));
 }
 result=ct_result(stats);return(int)result.status;
}
EX("metric") double metric(int field){
 switch(field){case 0:return result.n0;case 1:return result.n1;case 2:return result.mean0;case 3:return result.mean1;case 4:return result.variance0;case 5:return result.variance1;case 6:return result.t;default:return result.degenerate;}
}
