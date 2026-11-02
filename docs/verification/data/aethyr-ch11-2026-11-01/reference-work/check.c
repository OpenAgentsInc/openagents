#include "Work.h"
#include <stdint.h>
int main(void){
 uint8_t tags[]={2,3,128,255};
 for(unsigned i=0;i<sizeof tags;i++){coax_t_RRWorkLimits q=rr_probe_limits(tags[i]);if(q.ok||q.pass_steps||q.invocation_steps)return 1;}
 coax_t_RRWorkLimits a=rr_probe_limits(0),b=rr_probe_limits(1);
 if(!a.ok||a.pass_steps!=100000000||a.invocation_steps!=100000000||!b.ok||b.pass_steps!=1000000000||b.invocation_steps!=100000000)return 2;
 int64_t n[]={INT64_MIN,-1,0,1,99999999,100000000,100000001,1000000000,1000000001,INT64_MAX};
 int64_t e[]={-1,-1,0,1,99999999,100000000,100000000,100000000,-1,-1};
 for(unsigned i=0;i<sizeof n/sizeof*n;i++)if(rr_probe_invocation(n[i])!=e[i])return 3;
 return 0;
}
