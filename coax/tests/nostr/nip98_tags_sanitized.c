/* Test-only raw tag parser checks; no event verification or authorization. */
#include "tags.h"
#include <assert.h>
#include <stdint.h>
#include <string.h>
#define S(s) (coax_string){(s), (int64_t)(sizeof(s)-1)}
static char large[262145];
int main(void) {
  uint8_t u[1026],m[18],p[66];memset(u,165,sizeof u);memset(m,165,sizeof m);memset(p,165,sizeof p);
  coax_v_u8 uv={u+1,1024},mv={m+1,16},pv={p+1,64};
  coax_t_N98SourceTags a=n98_source_tags(S("[[\"u\",\"url\"],[\"method\",\"POST\"]]"),uv,mv,pv,0);
  assert(a.valid&&a.uc==1&&a.mc==1&&a.pc==0&&a.un==3&&a.mn==4);
  assert(!memcmp(u+1,"url",3)&&!memcmp(m+1,"POST",4));
  assert(u[0]==165&&u[1025]==165&&m[0]==165&&m[17]==165&&p[0]==165&&p[65]==165);
  assert(!n98_source_tags(S("[[\"u\",\"abc\"]]"),(coax_v_u8){u+1,2},mv,pv,0).valid);
  assert(!n98_source_tags(S("[[\"u\",\"x\",\"extra\"]]"),uv,mv,pv,0).valid);
  assert(n98_source_tags(S("[[\"unknown\",\"\\ud800\"]]"),uv,mv,pv,0).valid);
  assert(!n98_source_tags(S("[[\"u\",\"\\ud800\"]]"),uv,mv,pv,0).valid);
  assert(!n98_source_tags(S("[]"),uv,mv,pv,1).valid);
  assert(!n98_source_tags((coax_string){(const char *)0,-1},uv,mv,pv,0).valid);
  memset(large,' ',sizeof large);large[0]='[';large[1]=']';
  assert(n98_source_tags((coax_string){large,262144},uv,mv,pv,0).valid);
  assert(!n98_source_tags((coax_string){large,262145},uv,mv,pv,0).valid);
  return 0;
}
