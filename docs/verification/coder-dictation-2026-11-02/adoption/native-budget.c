// Independent native oracle for the adopted compiler work boundary.
// Build this translation unit against the committed generated checker seed.
#include "../../../../coax/bootstrap/checker.c"
#include <assert.h>
#include <stdio.h>

int main(void) {
  const int64_t sizes[] = {-1, 0, 1, 127, 128, 16384, 2147483646LL, 2147483647LL};
  int cases = 0;
  for (unsigned i = 0; i < sizeof sizes / sizeof sizes[0]; ++i) {
    coax_t_CK_BorrowCaps a = CK_borrow_caps(sizes[i]);
    coax_t_CK_RetainCaps b = CK_retain_caps(sizes[i]);
    const int valid = sizes[i] >= 1 && sizes[i] <= 2147483646LL;
    assert(a.work == (valid ? 2097152 : 0));
    assert(b.work == (valid ? 2097152 : 0));
    ++cases;
  }
  for (int fallback = 0; fallback < 2; ++fallback) {
    coax_t_CK_State state = {0};
    coax_t_CB_State work = {0};
    coax_t_CK_Error errors[2] = {0};
    coax_t_CB_Context b = {0};
    b.c.state = (coax_v_t_CK_State){&state, 1};
    b.c.errors = (coax_v_t_CK_Error){errors, 2};
    b.state = (coax_v_t_CB_State){&work, 1};
    work.work = 2097151;
    work.loc = (coax_t_CK_Loc){.line = 17, .column = 5, .offset = 17005};
    const coax_t_CK_Loc direct = {.line = 23, .column = 7, .offset = 23007};
    const coax_t_CK_Loc where = fallback ? (coax_t_CK_Loc){0} : direct;
    assert(coax_p_CB_tick(b, where));
    assert(work.work == 2097152 && !state.fatal && state.error_count == 0);
    assert(!coax_p_CB_tick(b, where));
    assert(work.work == 2097153 && state.fatal && state.error_count == 1);
    assert(errors[0].code.count == 5 && memcmp(errors[0].code.data, "E0381", 5) == 0);
    assert(errors[0].loc.line == (fallback ? 17 : 23));
    assert(errors[0].loc.offset == (fallback ? 17005 : 23007));
    assert(!coax_p_CB_tick(b, where));
    assert(work.work == 2097153 && state.error_count == 1);
    ++cases;
  }
  printf("native shared-work boundary: %d cases PASS\n", cases);
  return 0;
}
