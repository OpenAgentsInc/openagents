/* Chapter 231 against the independent oracle in soft_oracle.py. */
#include "soft-spans.h"
#include "soft-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>

static coax_v_s64 sv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}

int main(void){
    size_t exact=0,reads=0;
    for(size_t c=0;c<sizeof(soft_cases)/sizeof(*soft_cases);c++){
        const soft_case *s=&soft_cases[c];int64_t k[24],v[24];
        for(int i=0;i<s->n;i++){k[i]=s->key[i];v[i]=s->value[i];}
        coax_t_SmRead r=sm_read(sv(k,s->n),sv(v,s->n),s->n,s->q,s->beta,(coax_t_SmFormat)s->format);
        assert(r.status==COAX_SmStatus_ok && r.value==s->expect && (int)r.exact==s->exact);
        assert(fabs(r.mixed-s->mixed)<=1e-9*(1+fabs(s->mixed)));
        reads++;exact+=r.exact;
    }
    static int64_t keys[4096],values[4096];
    for(size_t c=0;c<sizeof(soft_limits)/sizeof(*soft_limits);c++){
        const soft_limit *l=&soft_limits[c];
        for(int i=0;i<4096;i++)keys[i]=values[i]=0;
        coax_t_SmLimit r=psionic_sm_limit(l->k,l->v,l->beta,(coax_t_SmFormat)l->format,sv(keys,4096),sv(values,4096));
        assert(r.status==COAX_SmStatus_ok && r.same.until==l->same_until && r.same.failure==l->same_failure);
        assert(r.near.until==l->near_until && r.near.failure==l->near_failure);
    }
    /* Overlapping scratch is refused before the scan writes anything. */
    for(int i=0;i<8;i++)keys[i]=-9;
    assert(psionic_sm_limit(16,1000,9.0,COAX_SmFormat_f64,sv(keys,4),sv(keys+2,4)).status==COAX_SmStatus_alias);
    for(int i=0;i<8;i++)assert(keys[i]==-9);
    printf("soft memory: %zu reads agree with the oracle (%zu exact); %zu limit scans agree\n",
        reads,exact,sizeof(soft_limits)/sizeof(*soft_limits));
    return 0;
}
