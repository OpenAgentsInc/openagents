/* Pthread effects for exact Coax TDT rows. Dispatch is synchronous and
 * never retains its views; start/stop serialize with every dispatch. */
#ifndef PSIONIC_PARAKEET_THREADS_H
#define PSIONIC_PARAKEET_THREADS_H
int pk_threads_start(int requested);
void pk_threads_stop(void);
int pk_threads_count(void);
#endif
