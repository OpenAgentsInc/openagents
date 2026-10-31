/* Independent zero-angle endpoint constants; no SDK or alternate FK program. */
#include "panda-token.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t checks;
#define NEED(v) do{checks++;if(!(v)){fprintf(stderr,"Panda token native line%d\n",__LINE__);return 1;}}while(0)
int main(int argc,char**argv){
 NEED(argc==2&&(!strcmp(argv[1],"0")||!strcmp(argv[1],"1")));int mutated=argv[1][0]=='1';double words[208];pfs_sample((coax_v_f64){words,208});for(int i=0;i<208;i++)NEED(isfinite(words[i]));
 const double gold[6]={-.662,0,1.8385,-.662,0,1.7415};const int index[6]={144,145,146,192,193,194};
 for(int i=0;i<6;i++){double expected=mutated?-gold[i]:gold[i];NEED(fabs(words[index[i]]-expected)<=2e-12);}
 NEED(checks==216);NEED(fwrite(words,sizeof(double),208,stdout)==208);return fflush(stdout)!=0;
}
