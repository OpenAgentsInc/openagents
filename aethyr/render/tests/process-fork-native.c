/* OS effect only: create a child in the caller's existing process group.
   Fixture modes, clocks, publication, process cleanup and verdicts are Coax. */
#include <stdint.h>
#include <unistd.h>
int64_t rpt_fork(void) { return (int64_t)fork(); }
