/* Test-only direct observer calls: no event verification or authorization. */
#include "observe.h"
#include <assert.h>
#include <stdint.h>
#define S(s) (coax_string){(s), (int64_t)(sizeof(s) - 1)}
int main(void) {
  coax_string u=S("u"),post=S("POST"),empty=S(""),lower=S("post");
  coax_string a=S("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  assert(n98_observe(27235,1000,1000,u,post,empty,u,lower,a,1,1,0,0));
  assert(n98_observe(27235,INT64_MAX,INT64_MAX,u,post,empty,u,lower,a,1,1,0,0));
  assert(!n98_observe(27235,0,INT64_MAX,u,post,empty,u,lower,a,1,1,0,0));
  assert(!n98_observe(27235,INT64_MAX,0,u,post,empty,u,lower,a,1,1,0,0));
  assert(!n98_observe(27235,-1,0,u,post,empty,u,lower,a,1,1,0,0));
  assert(n98_observe(27235,1000,1000,u,post,a,u,lower,a,1,1,1,0));
  assert(!n98_observe(27235,1000,1000,u,post,empty,u,lower,a,1,1,1,0));
  assert(!n98_observe(27235,1000,1000,u,post,a,u,lower,a,2,1,1,0));
  assert(!n98_observe(27235,1000,1000,u,post,a,u,lower,a,1,1,1,1));
  return 0;
}
