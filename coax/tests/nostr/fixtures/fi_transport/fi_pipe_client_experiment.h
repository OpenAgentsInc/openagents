#ifndef FI_PIPE_CLIENT_EXPERIMENT_H
#define FI_PIPE_CLIENT_EXPERIMENT_H
#include <stdint.h>
/* Test-only untrusted bytes. Caller serializes poll/free. No FI authority. */
typedef struct FiPipeExperiment FiPipeExperiment;
FiPipeExperiment *fi_pipe_experiment_start(const char *host,int port,int64_t timeout_ms);
int fi_pipe_experiment_poll(FiPipeExperiment *f,uint8_t *out,int64_t capacity,int64_t *written);
void fi_pipe_experiment_free(FiPipeExperiment *f);
#endif
