/* Verification-only effects. The original quantified observer still owns the
 * property process, with Coax deciding traversal and all encoded bytes. The
 * additional replay path invokes one Coax-admitted original tuple, without
 * pretending that earlier cases ran. */
#define main coax_semantics_quantified_main
#include "native-forall-observer.c"
#undef main

#include "native-semantics-observer.h"

static coax_t_EP_Identity active_identity;

static void observe_trap(const char *what, int64_t what_count,
                         const char *source, int64_t source_count, int64_t line) {
  uint8_t bytes[4128];
  int64_t count = SM_encode_trap(active_identity, (coax_string){what, what_count},
      (coax_string){source, source_count}, line, (coax_v_u8){bytes, sizeof(bytes)});
  /* An incomplete write remains an incomplete raw observation. The generated
   * runtime retains its ordinary diagnostic and abort after this returns. */
  if (count < 0 || (uint64_t)count > sizeof(bytes)) return;
  if (fwrite(bytes, 1, (size_t)count, stdout) != (size_t)count) return;
  (void)fflush(stdout);
}

static int publish_replay(Coax_ForallProgress_v2 *region, uint64_t case_index,
                           uint32_t event, int64_t returned) {
  uint8_t bytes[COAX_FORALL_SLOT_BYTES];
  int64_t count = SM_encode_replay(active_identity, case_index, event, returned,
      (coax_v_u8){bytes, sizeof(bytes)});
  if (count < 0 || (uint64_t)count > sizeof(bytes)) return 0;
  int64_t slot = QE_slot(atomic_load_explicit(&region->committed, memory_order_acquire));
  if (slot < 0 || slot >= 2) return 0; /* Concrete host array extent only. */
  memset(region->slots[slot], 0, COAX_FORALL_SLOT_BYTES);
  memcpy(region->slots[slot], bytes, (size_t)count);
  atomic_store_explicit(&region->committed, (unsigned char)slot, memory_order_release);
  return 1;
}

int main(int argc, char **argv) {
  coax_string mode = argument(argc > 1 ? argv[1] : NULL);
  coax_string ordinal = argument(argc > 2 ? argv[2] : NULL);
  coax_string case_index = argument(argc > 4 ? argv[3] : NULL);
  coax_string path = argument(argc > 4 ? argv[4] : argc > 3 ? argv[3] : NULL);
  coax_t_SM_Request request = SM_request(argc, mode, ordinal, case_index, path);
  if (request.reason) return 64;
  if (request.mode == 1) return coax_semantics_quantified_main(argc, argv);

  _Atomic unsigned char probe = 0;
  if (!QE_native_profile(sizeof(void *), sizeof(long), CHAR_BIT, sizeof(probe),
                           atomic_is_lock_free(&probe))) return 69;
  int64_t count = -1;
  const Coax_VerificationCheck_v2 *table = coax_verification_checks_v2(&count);
  if (!QE_table(count, table != NULL) || !EP_select(count, request.ordinal)) return 67;
  const Coax_VerificationCheck_v2 *row = &table[request.ordinal];
  /* The ordinary property path below validates the full immutable catalog
   * before invoking any body. No borrowed name/source bytes are read here. */
  active_identity = check_fact(row, request.ordinal, 0).identity;
  if (request.mode == 2) {
    coax_verification_set_trap_v1(observe_trap);
    return coax_semantics_quantified_main(argc, argv);
  }

  Snapshot snapshot = {0};
  Coax_ForallProgress_v2 *region = NULL;
  int status = 65;
  if (!make_snapshot(table, count, &snapshot)) goto done;
  coax_t_QE_Check check = snapshot.checks[request.ordinal];
  active_identity = check.identity;
  coax_t_FD_Domain domains[64];
  uint64_t words[64];
  if ((uint64_t)check.binding_count > 64) { status = 66; goto done; }
  for (int64_t i = 0; i < check.binding_count; ++i)
    domains[i] = snapshot.bindings[check.binding_start + i].domain;
  coax_v_t_FD_Domain input = {domains, check.binding_count};
  coax_v_u64 tuple = {words, check.binding_count};
  if (!SM_case(check, input, request.case_index, tuple)) { status = 66; goto done; }
  region = open_progress(path.data);
  if (!region) { status = 70; goto done; }
  status = 71;
  if (!publish_replay(region, request.case_index, 1, 0)) goto done;
  coax_verification_set_trap_v1(observe_trap);
  int64_t returned = row->run_values(tuple.data, tuple.count);
  if (!publish_replay(region, request.case_index, 2, returned)) goto done;
  status = 0;
done:
  dispose(&snapshot);
  if (region) munmap(region, sizeof(*region));
  return status;
}
