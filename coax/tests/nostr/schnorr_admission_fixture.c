#include "sign-admission.h"
#include <stdint.h>
static coax_t_SignSpan spans[6];
void sig_set(int i,uint64_t address,int64_t length){if(i>=0&&i<6){spans[i].address=address;spans[i].length=length;}}
int sig_check(int narrow){return sig_admit((coax_v_t_SignSpan){spans,6},narrow);}
