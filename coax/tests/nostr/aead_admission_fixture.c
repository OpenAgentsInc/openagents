#include "aead-admission.h"
#include <stdint.h>
static coax_t_AeadSpan spans[7];
void aa_set(int32_t i,uint64_t address,int64_t length){if(i>=0&&i<7)spans[i]=(coax_t_AeadSpan){.address=address,.length=length};}
int aa_check(int seal,int narrow){return aa_admit((coax_v_t_AeadSpan){spans,7},seal!=0,narrow!=0);}
