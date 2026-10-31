// Firm reason: 1 (file/argv/stdout transport for checked Coax release policy).
#include "changelog.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
  if (argc != 5) return 2;
  FILE *input = fopen(argv[1], "rb");
  if (!input) {
    fprintf(stderr, "release/changelog: cannot read %s\n", argv[1]);
    return 1;
  }
  // Read one byte beyond the Coax limit so oversized input is refused there.
  const size_t capacity = 262145;
  char *bytes = malloc(capacity);
  if (!bytes) { fclose(input); return 1; }
  size_t count = fread(bytes, 1, capacity, input);
  int failed = ferror(input);
  if (fclose(input) != 0) failed = 1;
  if (failed) { free(bytes); return 1; }
  coax_string text = {bytes, (int64_t)count};
  coax_string version = {argv[2], (int64_t)strlen(argv[2])};
  coax_string commit = {argv[3], (int64_t)strlen(argv[3])};
  coax_t_ChangelogPlan plan = CL_plan(text, version, commit, strcmp(argv[4], "--write") == 0);
  if (!plan.ok) {
    fprintf(stderr, "release/changelog: require a newest %s entry with a valid date, commit and authored notes in coax/CHANGELOG.md\n"
                    "Prepare with Commit: pending and --write; after committing the version bump, rerun --write to record its hash.\n", argv[2]);
    free(bytes);
    return 1;
  }
  if (plan.replace) {
    fwrite(bytes, 1, (size_t)plan.from, stdout);
    CL_render_commit(commit);
    fwrite(bytes + plan.to, 1, count - (size_t)plan.to, stdout);
  } else {
    fwrite(bytes, 1, count, stdout);
  }
  free(bytes);
  return fflush(stdout) == 0 && !ferror(stdout) ? 0 : 1;
}
