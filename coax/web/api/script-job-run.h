// The compile job run on one source (GitHub #13467; docs/protocol/world-scripts-v1.md, "The compile job"):
// shared by the API (scripts.c, through OA_SCRIPT_JOB) and the compile service (coax/web/script-job/serve.c,
// which the API reaches through OA_SCRIPT_JOB_URL), so both run the job the same way.
#ifndef SCRIPT_JOB_RUN_H
#define SCRIPT_JOB_RUN_H
#include <stdbool.h>
#include <stddef.h>

#define SCRIPT_JOB_WORDS 600       // a refusal's words (store.coax SS_WORDS)
#define SCRIPT_JOB_MODULE 65536    // a module's bytes (wasm.coax WM_BYTES)
#define SCRIPT_JOB_MANIFEST 8192   // a manifest's (manifest.coax SM_TEXT)

// Whether a name may name the job's files: 1 to 48 of a-z, 0-9 and '-'. The job holds a name to its own rule
// (script-job.coax sj_name_ok, a letter first); this only keeps a path in its directory.
bool script_job_name_safe(const char *name);
// words[0..n) made fit to say: line ends and spaces at the end cut, any byte that is not printable ASCII a
// space, terminated. n is clipped to cap - 1.
void script_job_words(char *words, long n, size_t cap);
// The command cmd (SOURCE NAME OUTDIR) run on a source in a directory of its own under TMPDIR, with only
// the environment that says where its tools are, for at most wait_ms. 0 with the module (at most
// SCRIPT_JOB_MODULE bytes) and the job's manifest (at most SCRIPT_JOB_MANIFEST) read back; 1 with the job's
// words; 2 when it could not be run, made nothing or ran past wait_ms (the words say which).
int script_job_run(const char *cmd, const char *name, const char *source, size_t source_n, long wait_ms,
                   char *wasm, long *wasm_n, char *made, long *made_n, char *words, size_t words_cap);
#endif
