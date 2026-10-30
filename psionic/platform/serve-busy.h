#ifndef PSIONIC_SERVE_BUSY_H
#define PSIONIC_SERVE_BUSY_H
#include <stdint.h>
#include <stdbool.h>
/* Generated serve.h defines SvLimits. Only refused sockets are retained;
 * there is no model job or request-byte queue. Coax owns the bounds. */
typedef struct {int fd[16];int64_t since[16];} PsBusy;
void psionic_busy_init(PsBusy *);
int psionic_busy_sweep(PsBusy *,int listener,bool active,coax_t_SvLimits,void (*observed)(void *),void *);
int psionic_busy_wait(const PsBusy *);
void psionic_busy_close(PsBusy *);
#endif
