#include "Work.h"
#include <stdint.h>
#ifdef RR_UNSIGNED_LOSS
#define INVOKE(x) rr_probe_invocation((uint32_t)(x))
#else
#define INVOKE(x) rr_probe_invocation(x)
#endif
int main(void){
 uint8_t tags[]={2,4,128,255};
 for(unsigned i=0;i<sizeof tags;i++){coax_t_RRWorkLimits q=rr_probe_limits(tags[i]);if(q.ok||q.pass_steps||q.invocation_steps)return 1;}
 int64_t budgets[]={100000000,1000000000,0,INT64_C(3000000000)};
 for(uint8_t tag=0;tag<4;tag++)if(tag!=2){coax_t_RRWorkLimits q=rr_probe_limits(tag);
#ifdef RR_SIGNED_LOSS
 q.pass_steps=(int32_t)q.pass_steps;
#endif
 if(!q.ok||q.pass_steps!=budgets[tag]||q.invocation_steps!=100000000)return 2;
 }
 int64_t n[]={INT64_MIN,-1,0,1,99999999,100000000,100000001,1000000001,2147483647,2147483648,2999999999,3000000000,3000000001,4294967297,INT64_MAX};
 int64_t e[]={-1,-1,0,1,99999999,100000000,100000000,100000000,100000000,100000000,100000000,100000000,-1,-1,-1};
 for(unsigned i=0;i<sizeof n/sizeof*n;i++)if(INVOKE(n[i])!=e[i])return 3;
 int64_t counter=INT64_C(2147483647);counter++;if(counter!=INT64_C(2147483648)||INVOKE(counter)!=100000000)return 4;
 return 0;
}
