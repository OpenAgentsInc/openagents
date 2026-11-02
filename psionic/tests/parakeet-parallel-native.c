/* Independent native effect oracle, Firm reason 2: invoke pthread and sanitizer
 * behavior unavailable in the Coax VM. Numeric oracles remain in Coax. */
#include "parakeet-parallel.h"
#include "parakeet-threads.h"
#include <stdio.h>
#include <pthread.h>
#include <unistd.h>
static void *client(void *p){int *rc=p;*rc=(int)pkp_native_check((uint64_t)(*rc));return NULL;}
int main(void){
 int requests[]={1,2,8,0,1,8};
 for(unsigned k=0;k<sizeof(requests)/sizeof(requests[0]);k++){
  int n=pk_threads_start(requests[k]);
  if(n<1||n>8||pk_threads_count()!=n){fprintf(stderr,"thread count mismatch\n");return 1;}
#ifdef PK_THREADS_TEST_FAIL_AFTER
  if(n>PK_THREADS_TEST_FAIL_AFTER+1)return 5;
  if(requests[k]>=8 && sysconf(_SC_NPROCESSORS_ONLN)>PK_THREADS_TEST_FAIL_AFTER+1 && n!=PK_THREADS_TEST_FAIL_AFTER+1)return 6;
#endif
  if(pkp_native_rows()){fprintf(stderr,"row parity failed, threads=%d\n",n);return 2;}
  for(int seed=0;seed<6;seed++){int rc=(int)pkp_native_check(seed);if(rc){fprintf(stderr,"decoder parity failed seed=%d threads=%d rc=%d\n",seed,n,rc);return 3;}}
  printf("bitwise tokens/frames/workspace + row/tail/refusal parity: requested=%d actual=%d seeds=6\n",requests[k],n);
  pk_threads_stop();if(pk_threads_count()!=1)return 4;
 }
 pk_threads_start(8);int a=0,b=3;pthread_t x,y;pthread_attr_t attr;
 // This oracle deliberately owns 0.8MB of local weights per client. Apple's
 // default pthread stack is only 512KB; product row workers use no such array.
 if(pthread_attr_init(&attr)||pthread_attr_setstacksize(&attr,4*1024*1024))return 7;
 if(pthread_create(&x,&attr,client,&a)||pthread_create(&y,&attr,client,&b))return 7;
 pthread_attr_destroy(&attr);
 pthread_join(x,NULL);pthread_join(y,NULL);if(a||b)return 8;
 pk_threads_stop();puts("thread lifecycle and repeated close: PASS");return 0;
}
