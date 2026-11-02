/* Pthread effects only. Coax owns row partitions, counts and arithmetic. */
#include "parakeet-parallel.h"
#include "parakeet-threads.h"
#include <pthread.h>
#include <stdint.h>
#include <unistd.h>
#include <errno.h>
static pthread_mutex_t run_mu=PTHREAD_MUTEX_INITIALIZER,mu=PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t wake=PTHREAD_COND_INITIALIZER,done=PTHREAD_COND_INITIALIZER;
static pthread_t workers[7];
static int count,stop,pending,bad;static unsigned long generation;static coax_t_PkpJob current;
static void *worker(void *arg){int index=(int)(intptr_t)arg;unsigned long seen=0;pthread_mutex_lock(&mu);
 for(;;){while(!stop&&generation==seen)pthread_cond_wait(&wake,&mu);if(stop)break;seen=generation;coax_t_PkpJob j=current;int total=count+1;pthread_mutex_unlock(&mu);
  coax_t_PkpRange rows=pkp_range(j.rows,total,index);int ok=rows.ok&&pkp_dot_part(j,rows.lo,rows.hi);
  pthread_mutex_lock(&mu);if(!ok)bad=1;pending--;if(!pending)pthread_cond_signal(&done);
 }pthread_mutex_unlock(&mu);return NULL;
}
static void stop_locked(void){pthread_mutex_lock(&mu);stop=1;pthread_cond_broadcast(&wake);pthread_mutex_unlock(&mu);for(int i=0;i<count;i++)pthread_join(workers[i],NULL);pthread_mutex_lock(&mu);count=0;stop=0;generation=0;pending=0;pthread_mutex_unlock(&mu);}
int pk_threads_start(int requested){pthread_mutex_lock(&run_mu);stop_locked();long available=sysconf(_SC_NPROCESSORS_ONLN);int total=(int)pkp_threads(requested,available);pthread_mutex_lock(&mu);
 for(int i=1;i<total;i++){
#ifdef PK_THREADS_TEST_FAIL_AFTER
  int error=i>PK_THREADS_TEST_FAIL_AFTER?EAGAIN:pthread_create(&workers[count],NULL,worker,(void*)(intptr_t)i);
#else
  int error=pthread_create(&workers[count],NULL,worker,(void*)(intptr_t)i);
#endif
  if(error)break;count++;
 }int actual=count+1;pthread_mutex_unlock(&mu);pthread_mutex_unlock(&run_mu);return actual;
}
void pk_threads_stop(void){pthread_mutex_lock(&run_mu);stop_locked();pthread_mutex_unlock(&run_mu);}
int pk_threads_count(void){pthread_mutex_lock(&run_mu);int n=count+1;pthread_mutex_unlock(&run_mu);return n;}
_Bool cxpk_dot(int64_t rows_count,int64_t cols,coax_v_f32 w,coax_v_f32 x,coax_v_f32 bias,_Bool second,coax_v_f32 w2,coax_v_f32 x2,coax_v_f32 bias2,coax_v_f32 output){coax_t_PkpJob j={.rows=rows_count,.cols=cols,.w=w,.x=x,.bias=bias,.second=second,.w2=w2,.x2=x2,.bias2=bias2,.out=output};pthread_mutex_lock(&run_mu);pthread_mutex_lock(&mu);current=j;pending=count;bad=0;int total=count+1;generation++;pthread_cond_broadcast(&wake);pthread_mutex_unlock(&mu);
 coax_t_PkpRange rows=pkp_range(j.rows,total,0);int ok=rows.ok&&pkp_dot_part(j,rows.lo,rows.hi);
 pthread_mutex_lock(&mu);while(pending)pthread_cond_wait(&done,&mu);ok=ok&&!bad;pthread_mutex_unlock(&mu);pthread_mutex_unlock(&run_mu);return ok;
}
