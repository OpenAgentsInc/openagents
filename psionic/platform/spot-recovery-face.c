/* Process I/O only: the generated Coax procedure decides the action. */
#include "spot-recovery.h"
#include <stdio.h>
#include <string.h>
static coax_string text(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
int main(int argc,char **argv){if(argc!=6)return 64;printf("%lld\n",(long long)spot_recovery_action(text(argv[1]),text(argv[2]),text(argv[3]),text(argv[4]),text(argv[5])));return 0;}
