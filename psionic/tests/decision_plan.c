#include "decision-plan.h"
#include <assert.h>
#include <stdio.h>
int main(void){
    coax_t_PsPlan e=ps_plan(0,86,86,49),m=ps_plan(1,86,86,49);
    assert(e.status==COAX_PsPlanStatus_ok && e.tensors==760 && e.layers==32 && e.params==1057280 && e.state==14581760 && e.weight_bytes==18819722392LL);
    assert(m.status==COAX_PsPlanStatus_ok && m.tensors==1184 && m.layers==64 && m.params==2640384 && m.layer_work==62176 && m.norm==5120 && m.state==42041344 && m.weight_bytes==54713608144LL);
    assert(m.work>e.work && ps_plan(1,128,86,49).state==43417600);
    assert(ps_plan(2,86,86,49).status==COAX_PsPlanStatus_target && ps_plan(1,0,1,0).status==COAX_PsPlanStatus_target && ps_plan(1,16385,1,0).status==COAX_PsPlanStatus_target);
    assert(ps_plan(1,86,87,49).status==COAX_PsPlanStatus_request && ps_plan(1,86,86,86).status==COAX_PsPlanStatus_request && ps_plan(1,86,86,-1).status==COAX_PsPlanStatus_request && ps_plan(1,86,0,0).status==COAX_PsPlanStatus_request);
    puts("decision plan native: pinned target storage and request bounds pass");
}
