#include <stdint.h>
typedef void (*Coax_VerificationTrapHandler_v1)(const char *,int64_t,const char *,int64_t,int64_t);
extern void coax_verification_set_trap_v1(Coax_VerificationTrapHandler_v1);
int main(void) { coax_verification_set_trap_v1(0); return 0; }
