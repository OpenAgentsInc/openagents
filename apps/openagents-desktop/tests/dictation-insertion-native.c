/* Independent native C ABI witness; numeric/text decisions stay in Coax. */
#include "dictation-insertion.h"
#include <stdio.h>
int main(void) {
  int result = (int)dictation_insertion_native();
  if (result) { fprintf(stderr, "guarded draft insertion failed case %d\n", result); return result; }
  puts("guarded draft insertion, undo, stale/cancel, Unicode refusal and no-send: PASS");
  return 0;
}
