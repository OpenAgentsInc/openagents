#include "decision-quality-memory.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
int main(void){
    double p[11]={0.8,0.1,0.1, 0.3,0.7, 0,0.5,0.5, 0.1,0.2,0.7};
    coax_t_PqItem items[4]={{COAX_QrKind_choice,3,0,0,2},{COAX_QrKind_noul,2,3,0,-1},{COAX_QrKind_score,3,5,2,-1},{COAX_QrKind_choice,3,8,2,2}};
    coax_t_PqReport r=psionic_quality((coax_v_t_PqItem){items,4},(coax_v_f64){p,11});
    assert(r.status==COAX_PqStatus_ok && r.items==4 && r.correct==2 && fabs(r.brier-0.42)<1e-12 && fabs(r.ece-0.275)<1e-12);
    assert(r.kinds.e[0].items==2 && r.kinds.e[1].correct==0 && fabs(r.kinds.e[2].brier-0.5)<1e-12 && fabs(r.score_error-0.5)<1e-12);
    assert(r.offered==2 && r.abstained==1 && r.answered==3 && r.answered_correct==1);
    p[4]=NAN;r=psionic_quality((coax_v_t_PqItem){items,4},(coax_v_f64){p,11});assert(r.status==COAX_PqStatus_number && r.cause==1);
    p[4]=0.7;items[3].kind=(coax_t_QrKind)3;r=psionic_quality((coax_v_t_PqItem){items,4},(coax_v_f64){p,11});assert(r.status==COAX_PqStatus_shape && r.cause==3);
    items[3].kind=COAX_QrKind_choice;items[3].start=INT64_MAX;r=psionic_quality((coax_v_t_PqItem){items,4},(coax_v_f64){p,11});assert(r.status==COAX_PqStatus_shape && r.cause==3);
    items[3].start=8;items[3].count=INT64_MIN;r=psionic_quality((coax_v_t_PqItem){items,4},(coax_v_f64){p,11});assert(r.status==COAX_PqStatus_shape);
    puts("quality native: hand-derived metrics, NaN, kind and extreme offset refusals pass");
}
