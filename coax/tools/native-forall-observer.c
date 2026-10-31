/* Verification ABI2 host effects. Coax owns table admission, finite traversal,
 * frame bytes and progress meaning; this adapter owns allocation, mmap, stdio,
 * atomic publication and the original checked procedure's function-pointer call. */
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

#include "native-forall-observer.h"
#include "native-forall-progress.h"
#include "quantified-check-execution-policy.h"

typedef struct Snapshot {
  int64_t check_count, binding_count, member_count;
  coax_t_QE_Check *checks;
  coax_t_QE_Binding *bindings;
  coax_t_QE_Member *members;
  uint64_t *words;
} Snapshot;

static coax_string argument(const char *value) {
  return (coax_string){value, value ? (int64_t)strlen(value) : 0};
}

static void dispose(Snapshot *snapshot) {
  free(snapshot->checks);
  free(snapshot->bindings);
  free(snapshot->members);
  free(snapshot->words);
  memset(snapshot, 0, sizeof(*snapshot));
}

static void *storage(int64_t count, size_t size) {
  if (count < 0 || (uint64_t)count > SIZE_MAX / size) return NULL;
  return calloc(count ? (size_t)count : 1, size);
}

static coax_t_QE_Check check_fact(const Coax_VerificationCheck_v2 *row, int64_t ordinal,
                                int64_t binding_start) {
  coax_t_QE_Check result = {0};
  result.identity = (coax_t_EP_Identity){ordinal, row->declaration_index, row->line, row->column,
      {row->name, row->name_count}, {row->source, row->source_count}};
  result.kind = QE_kind(row->kind);
  result.cases = row->case_count;
  result.binding_start = binding_start;
  result.binding_count = row->binding_count;
  return result;
}

static coax_t_QE_Binding binding_fact(const Coax_VerificationBinding_v2 *binding,
                                    int64_t member_start, coax_v_u64 values) {
  coax_t_QE_Binding result = {0};
  result.name = (coax_string){binding->name, binding->name_count};
  result.type_name = (coax_string){binding->type_name, binding->type_name_count};
  result.domain = QE_domain(binding->kind, binding->width, binding->is_signed,
                            binding->first, binding->count, values);
  result.member_start = member_start;
  result.member_count = binding->value_count;
  return result;
}

static int make_snapshot(const Coax_VerificationCheck_v2 *table, int64_t count,
                         Snapshot *snapshot) {
  if (!QE_table(count, table != NULL)) return 0;
  int64_t bindings = 0, members = 0;
  /* Inspect scalar extents before dereferencing borrowed arrays or allocating.
   * Coax bounds each count, so the transport sums fit int64_t on LP64. */
  for (int64_t i = 0; i < count; ++i) {
    const Coax_VerificationCheck_v2 *row = &table[i];
    coax_t_QE_Check check = check_fact(row, i, bindings);
    if (!QE_check_valid(check, count, row->run_values != NULL, row->name != NULL,
                        row->source != NULL, row->bindings != NULL)) return 0;
    for (int64_t j = 0; j < row->binding_count; ++j) {
      const Coax_VerificationBinding_v2 *binding = &row->bindings[j];
      coax_t_QE_Binding fact = binding_fact(binding, members, (coax_v_u64){NULL, 0});
      if (!QE_binding_valid(fact, binding->name != NULL, binding->type_name != NULL,
                            binding->values != NULL)) return 0;
      members += binding->value_count;
    }
    bindings += row->binding_count;
  }
  uint8_t header[COAX_FORALL_SLOT_BYTES];
  if (QE_encode_header(count, bindings, members,
                        (coax_v_u8){header, sizeof(header)}) < 0) return 0;
  snapshot->check_count = count;
  snapshot->binding_count = bindings;
  snapshot->member_count = members;
  snapshot->checks = storage(count, sizeof(*snapshot->checks));
  snapshot->bindings = storage(bindings, sizeof(*snapshot->bindings));
  snapshot->members = storage(members, sizeof(*snapshot->members));
  snapshot->words = storage(members, sizeof(*snapshot->words));
  if (!snapshot->checks || !snapshot->bindings || !snapshot->members || !snapshot->words) return 0;
  bindings = 0;
  members = 0;
  for (int64_t i = 0; i < count; ++i) {
    const Coax_VerificationCheck_v2 *row = &table[i];
    snapshot->checks[i] = check_fact(row, i, bindings);
    for (int64_t j = 0; j < row->binding_count; ++j) {
      const Coax_VerificationBinding_v2 *binding = &row->bindings[j];
      coax_v_u64 values = {snapshot->words + members, binding->value_count};
      coax_t_QE_Binding fact = binding_fact(binding, members, (coax_v_u64){NULL, 0});
      for (int64_t k = 0; k < binding->value_count; ++k) {
        const Coax_VerificationEnumValue_v2 *member = &binding->values[k];
        coax_t_QE_Member item = {member->bits, {member->label, member->label_count}};
        if (!QE_member_valid(item, fact.domain.bits, fact.domain.is_signed,
                              member->label != NULL)) return 0;
        snapshot->members[members + k] = item;
        snapshot->words[members + k] = member->bits;
      }
      fact = binding_fact(binding, members, values);
      if (!QE_binding_valid(fact, binding->name != NULL, binding->type_name != NULL,
                            binding->values != NULL)) return 0;
      snapshot->bindings[bindings + j] = fact;
      members += binding->value_count;
    }
    bindings += row->binding_count;
  }
  return QE_catalog_valid((coax_v_t_QE_Check){snapshot->checks, count},
                            (coax_v_t_QE_Binding){snapshot->bindings, bindings},
                            (coax_v_t_QE_Member){snapshot->members, members});
}

static int output_frame(const uint8_t *buffer, int64_t count) {
  if (count < 0 || (uint64_t)count > COAX_FORALL_SLOT_BYTES) return 0;
  if (fwrite(buffer, 1, (size_t)count, stdout) != (size_t)count) return 0;
  return fflush(stdout) == 0;
}

static int output_catalog(const Snapshot *snapshot) {
  uint8_t buffer[COAX_FORALL_SLOT_BYTES];
  coax_v_u8 out = {buffer, sizeof(buffer)};
  if (!output_frame(buffer, QE_encode_header(snapshot->check_count, snapshot->binding_count,
                                             snapshot->member_count, out))) return 0;
  for (int64_t i = 0; i < snapshot->check_count; ++i)
    if (!output_frame(buffer, QE_encode_check(snapshot->checks[i], out))) return 0;
  for (int64_t i = 0; i < snapshot->binding_count; ++i)
    if (!output_frame(buffer, QE_encode_binding(snapshot->bindings[i], out))) return 0;
  for (int64_t i = 0; i < snapshot->member_count; ++i)
    if (!output_frame(buffer, QE_encode_member(snapshot->members[i], out))) return 0;
  return 1;
}

static Coax_ForallProgress_v2 *open_progress(const char *path) {
  int descriptor = open(path, O_RDWR | O_NOFOLLOW);
  if (descriptor < 0) return NULL;
  struct stat facts;
  if (fstat(descriptor, &facts) != 0 || !S_ISREG(facts.st_mode) ||
      facts.st_size != sizeof(Coax_ForallProgress_v2)) {
    close(descriptor);
    return NULL;
  }
  void *region = mmap(NULL, sizeof(Coax_ForallProgress_v2), PROT_READ | PROT_WRITE,
                       MAP_SHARED, descriptor, 0);
  close(descriptor);
  if (region == MAP_FAILED) return NULL;
  if (!QE_initial((coax_v_u8){region, sizeof(Coax_ForallProgress_v2)})) {
    munmap(region, sizeof(Coax_ForallProgress_v2));
    return NULL;
  }
  return region;
}

static int publish(Coax_ForallProgress_v2 *region, coax_t_QE_State state, coax_v_u64 tuple) {
  uint8_t buffer[COAX_FORALL_SLOT_BYTES];
  int64_t count = QE_encode_progress(state, tuple, (coax_v_u8){buffer, sizeof(buffer)});
  if (count < 0 || (uint64_t)count > sizeof(buffer)) return 0;
  int64_t slot = QE_slot(atomic_load_explicit(&region->committed, memory_order_acquire));
  if (slot < 0 || slot >= 2) return 0; /* Protect the concrete two-slot array. */
  memset(region->slots[slot], 0, COAX_FORALL_SLOT_BYTES);
  memcpy(region->slots[slot], buffer, (size_t)count);
  atomic_store_explicit(&region->committed, (unsigned char)slot, memory_order_release);
  return 1;
}

int main(int argc, char **argv) {
  coax_t_EP_Request request = QE_request(argc, argument(argc > 1 ? argv[1] : NULL),
      argument(argc > 2 ? argv[2] : NULL), argument(argc > 3 ? argv[3] : NULL));
  if (request.reason) return 64;
  _Atomic unsigned char probe = 0;
  if (!QE_native_profile(sizeof(void *), sizeof(long), CHAR_BIT, sizeof(probe),
                           atomic_is_lock_free(&probe))) return 69;
  Coax_ForallProgress_v2 *region = NULL;
  if (request.mode == 2) {
    region = open_progress(argv[3]);
    if (!region) return 70;
  }
  int64_t count = -1;
  const Coax_VerificationCheck_v2 *table = coax_verification_checks_v2(&count);
  Snapshot snapshot = {0};
  int status = 65;
  if (!make_snapshot(table, count, &snapshot)) goto done;
  if (request.mode == 1) {
    status = output_catalog(&snapshot) ? 0 : 68;
    goto done;
  }
  if (request.mode != 2 || !EP_select(count, request.ordinal)) { status = 67; goto done; }
  const Coax_VerificationCheck_v2 *row = &table[request.ordinal];
  coax_t_QE_Check check = snapshot.checks[request.ordinal];
  coax_t_FD_Domain domains[64];
  uint64_t words[64];
  /* These independent extent guards protect this concrete host stack storage. */
  if ((uint64_t)check.binding_count > 64) { status = 66; goto done; }
  for (int64_t i = 0; i < check.binding_count; ++i)
    domains[i] = snapshot.bindings[check.binding_start + i].domain;
  coax_v_t_FD_Domain input = {domains, check.binding_count};
  coax_v_u64 tuple = {words, check.binding_count};
  coax_t_QE_State state = QE_start(check);
  status = 71;
  while (QE_ready(state)) {
    state = QE_begin(state);
    if (state.reason || FD_tuple(input, state.case_index, tuple) != 0 || !publish(region, state, tuple)) goto done;
    int64_t returned = row->run_values(tuple.data, tuple.count);
    state = QE_end(state, returned);
    if (state.reason || !publish(region, state, tuple)) goto done;
  }
  state = QE_done(state);
  if (state.reason || !publish(region, state, tuple)) goto done;
  status = 0;
done:
  dispose(&snapshot);
  if (region) munmap(region, sizeof(*region));
  return status;
}
