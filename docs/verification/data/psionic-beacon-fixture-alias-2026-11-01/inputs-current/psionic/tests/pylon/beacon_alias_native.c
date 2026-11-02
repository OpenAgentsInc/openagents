#include "Probe.h"
#include <stdio.h>
int main(void) {
    coax_t_PylonBeaconAliasProbe q=pylon_beacon_alias_probe();
    printf("%lld %lld %lld %lld %lld %lld %lld\n",
           (long long)q.before_code,(long long)q.borrowed_code,(long long)q.stable_code,
           (long long)q.borrowed_count,(long long)q.stable_count,
           (long long)q.borrowed_tail,(long long)q.stable_tail);
    return !(q.before_code==COAX_PbCode_digest && q.borrowed_code==COAX_PbCode_tags &&
             q.stable_code==COAX_PbCode_digest && q.borrowed_count==148 && q.stable_count==148 &&
             q.borrowed_tail==',' && q.stable_tail==']');
}
