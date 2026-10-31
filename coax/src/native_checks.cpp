// Checked-AST native-check emission. Native code transports resolved facts
// and writes C; lib/compiler/native-checks.coax owns selection, eligibility
// and graph closure. The bootstrap compiler explicitly lacks this command.
#include "coax.h"
#include <algorithm>
#include <map>
#include <set>

namespace coax {

#ifndef COAX_BOOTSTRAP_IMPORTER
namespace {

// Native address observations only. Subtraction avoids overflowing either
// span end; empty views do not overlap. Extents come from live containers
// or the checked source's immutable string storage.
static bool nc_spans_overlap(const void *first, size_t first_size, const void *second, size_t second_size) {
  if (!first_size || !second_size) return false;
  auto a = reinterpret_cast<uintptr_t>(first), b = reinterpret_cast<uintptr_t>(second);
  return a <= b ? b - a < first_size : a - b < second_size;
}

// Structural type observations only. No lifetime or alias resolver is added.
static bool nc_borrowed_type(Type type, std::set<Type> &seen) {
  if (!type) return false;
  if (type->kind == TypeKind::view || type->kind == TypeKind::string_) return true;
  if (!seen.insert(type).second) return false;
  if (type->kind == TypeKind::array) return nc_borrowed_type(type->elem, seen);
  if (type->kind == TypeKind::struct_ && type->struct_decl)
    for (const Field &field : type->struct_decl->fields)
      if (nc_borrowed_type(field.type, seen)) return true;
  return false;
}

static bool nc_borrowed_type(Type type) {
  std::set<Type> seen;
  return nc_borrowed_type(type, seen);
}

struct NativeCheckFacts {
  const File *file;
  std::vector<const ProcDecl *> procedures;
  std::map<const ProcDecl *, int64_t> ids;
  std::vector<coax_t_NC_Proc> proc_facts;
  std::vector<coax_t_NC_Check> checks;
  std::vector<coax_t_NC_Edge> edges;
  std::vector<Loc> hazard_locations;
  std::set<std::pair<int64_t, int64_t>> call_edges;
  std::set<const Expr *> seen_expressions;
  std::set<Type> seen_defaults;
  int64_t current = -1;

  int64_t add(const ProcDecl *procedure) {
    auto found = ids.find(procedure);
    if (found != ids.end()) return found->second;
    int64_t id = (int64_t)procedures.size();
    ids.emplace(procedure, id);
    procedures.push_back(procedure);
    return id;
  }

  void hazard(uint32_t bits, Loc loc) {
    auto &fact = proc_facts[(size_t)current];
    if (!fact.hazards) hazard_locations[(size_t)current] = loc;
    fact.hazards |= bits;
  }

  void defaults(Type type) {
    if (!type || !seen_defaults.insert(type).second) return;
    if (type->kind == TypeKind::array) defaults(type->elem);
    if (type->kind == TypeKind::struct_ && type->struct_decl)
      for (const Field &field : type->struct_decl->fields) {
        expression(field.init);
        defaults(field.type);
      }
  }

  void expression(const Expr *expr) {
    if (!expr || !seen_expressions.insert(expr).second) return;
    if (!expr->type || expr->type->kind == TypeKind::invalid) hazard(8, expr->loc);
    switch (expr->kind) {
      case ExprKind::int_lit: case ExprKind::float_lit: case ExprKind::bool_lit:
      case ExprKind::string_lit: case ExprKind::enum_value:
        return;
      case ExprKind::ct_value:
        if (!expr->ct) hazard(8, expr->loc);
        if (nc_borrowed_type(expr->type)) hazard(4, expr->loc);
        return;  // A frozen value's former expression is not a runtime edge.
      case ExprKind::ident:
        if (expr->constant) expression(expr->constant->expr);
        return;
      case ExprKind::run:
        hazard(8, expr->loc);
        return;
      case ExprKind::call:
        if (expr->builtin_print) hazard(1, expr->loc);
        if (expr->byte_intrinsic == ByteIntrinsic::bytes_overlap) hazard(1, expr->loc);
        if (expr->proc) {
          auto target = ids.find(expr->proc);
          call_edges.emplace(current, target == ids.end() ? -1 : target->second);
        } else if (expr->intrinsic == Intrinsic::none && expr->byte_intrinsic == ByteIntrinsic::none && !expr->builtin_print) {
          hazard(1, expr->loc);
        }
        for (const Expr *argument : expr->args) expression(argument);
        return;
      case ExprKind::cast:
        if (expr->type == &T_STRING && expr->lhs && expr->lhs->type && expr->lhs->type->kind == TypeKind::view)
          hazard(2, expr->loc);
        expression(expr->lhs);
        return;
      case ExprKind::field: case ExprKind::unary:
        expression(expr->lhs);
        return;
      case ExprKind::binary: case ExprKind::index:
        expression(expr->lhs); expression(expr->rhs);
        return;
      case ExprKind::slice: case ExprKind::ifx:
        expression(expr->lhs); expression(expr->rhs); expression(expr->third);
        return;
      case ExprKind::struct_lit:
        defaults(expr->type);
        for (const FieldInit &field : expr->fields) expression(field.value);
        return;
      case ExprKind::array_lit:
        for (const Expr *element : expr->args) expression(element);
        return;
    }
    hazard(8, expr->loc);
  }

  void statement(const Stmt *stmt) {
    if (!stmt) return;
    if ((stmt->kind == StmtKind::assign || stmt->kind == StmtKind::ret) && stmt->expr && nc_borrowed_type(stmt->expr->type))
      hazard(4, stmt->loc);
    if (stmt->kind == StmtKind::var) {
      defaults(stmt->type);
      if (stmt->uninit) hazard(8, stmt->loc);
    }
    expression(stmt->expr); expression(stmt->expr2); expression(stmt->target);
    for (const Stmt *child : stmt->body) statement(child);
    statement(stmt->then_); statement(stmt->else_);
  }

  void collect() {
    for (const ProcDecl *procedure : file->procs) add(procedure);
    for (const CheckDecl *check : file->checks) if (check->proc) add(check->proc);
    proc_facts.resize(procedures.size());
    hazard_locations.resize(procedures.size());
    for (size_t i = 0; i < procedures.size(); ++i) {
      const ProcDecl *procedure = procedures[i];
      auto &fact = proc_facts[i];
      fact.name = {procedure->name.data, procedure->name.count};
      fact.parameter_count = procedure->params.count;
      fact.returns_void = procedure->ret == &T_VOID;
      fact.has_body = procedure->body != nullptr;
      fact.foreign = procedure->foreign;
      // The ordinary C emitter recognizes an authored main by this spelling.
      // Check display names live outside that procedure namespace.
      fact.product_main = std::find(file->procs.begin(), file->procs.end(), procedure) != file->procs.end() && str_eq(procedure->name, "main");
      hazard_locations[i] = procedure->loc;
      current = (int64_t)i;
      seen_expressions.clear(); seen_defaults.clear();
      if (nc_borrowed_type(procedure->ret)) hazard(4, procedure->loc);
      if (!procedure->ret || procedure->ret->kind == TypeKind::invalid) hazard(8, procedure->loc);
      statement(procedure->body);
    }
    for (const CheckDecl *check : file->checks) {
      auto found = ids.find(check->proc);
      coax_t_NC_Check fact{};
      fact.name = {check->name.data, check->name.count};
      fact.kind = check->forall ? 2 : 1;
      fact.procedure = found == ids.end() ? -1 : found->second;
      checks.push_back(fact);
    }
    for (const auto &edge : call_edges) edges.push_back({edge.first, edge.second});
  }

  bool observe_overlap(const std::vector<uint8_t> &procedure_mask, const std::vector<uint8_t> &check_mask,
                       coax_string selection) const {
    auto writable_overlap = [&](const void *data, size_t size) {
      return nc_spans_overlap(procedure_mask.data(), procedure_mask.size(), data, size) ||
             nc_spans_overlap(check_mask.data(), check_mask.size(), data, size);
    };
    // A live vector's allocated element count bounds these byte products.
    if (nc_spans_overlap(procedure_mask.data(), procedure_mask.size(), check_mask.data(), check_mask.size()) ||
        writable_overlap(proc_facts.data(), proc_facts.size() * sizeof(coax_t_NC_Proc)) ||
        writable_overlap(checks.data(), checks.size() * sizeof(coax_t_NC_Check)) ||
        writable_overlap(edges.data(), edges.size() * sizeof(coax_t_NC_Edge)) ||
        writable_overlap(selection.data, selection.count > 0 ? (size_t)selection.count : 0)) return true;
    for (const auto &procedure : proc_facts)
      if (writable_overlap(procedure.name.data, procedure.name.count > 0 ? (size_t)procedure.name.count : 0)) return true;
    for (const auto &check : checks)
      if (writable_overlap(check.name.data, check.name.count > 0 ? (size_t)check.name.count : 0)) return true;
    return false;
  }
};

static Loc nc_refusal_location(const NativeCheckFacts &facts, uint32_t reason, int64_t index) {
  if ((reason == 4 || reason == 7 || reason == 8) && index >= 0 && index < facts.file->checks.count)
    return facts.file->checks[(int32_t)index]->loc;
  if ((reason == 3 || (reason >= 9 && reason <= 15)) && index >= 0 && (size_t)index < facts.procedures.size())
    return reason >= 12 ? facts.hazard_locations[(size_t)index] : facts.procedures[(size_t)index]->loc;
  if (reason == 5 && index >= 0 && (size_t)index < facts.edges.size()) {
    int64_t from = facts.edges[(size_t)index].from;
    if (from >= 0 && (size_t)from < facts.procedures.size()) return facts.procedures[(size_t)from]->loc;
  }
  return {1, 1, 0};
}

}  // namespace
#endif

bool emit_native_checks_c(const File *file, const EmitOptions &opts, const char *only, std::string *text) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)opts; (void)only; (void)text;
  error_at(file->src, {1, 1, 0}, "E0413", "the bootstrap compiler has no native-check emission policy");
  return false;
#else
  NativeCheckFacts facts{file};
  facts.collect();
  std::vector<uint8_t> procedure_mask(facts.procedures.size()), check_mask(facts.checks.size());
  uint32_t profile = (opts.wrapping ? 1u : 0u) | (opts.metal ? 2u : 0u) | (opts.cuda ? 4u : 0u) | (opts.metal_compute ? 8u : 0u);
  coax_string selection{only ? only : "", only ? (int64_t)strlen(only) : 0};
  bool overlap = facts.observe_overlap(procedure_mask, check_mask, selection);
  coax_t_NC_Result result = NC_plan({facts.proc_facts.data(), (int64_t)facts.proc_facts.size()},
      {facts.checks.data(), (int64_t)facts.checks.size()}, {facts.edges.data(), (int64_t)facts.edges.size()},
      selection, only != nullptr, profile, overlap, {procedure_mask.data(), (int64_t)procedure_mask.size()},
      {check_mask.data(), (int64_t)check_mask.size()});
  if (result.reason) {
    coax_string reason = NC_reason_text(result.reason);
    error_at(file->src, nc_refusal_location(facts, result.reason, result.index), "E0413",
             std::string(reason.data, (size_t)reason.count) + " is outside checked native-check emission");
    return false;
  }
  NativeCheckCPlan plan;
  for (size_t i = 0; i < facts.procedures.size(); ++i)
    if (procedure_mask[i]) plan.procedures.push_back(facts.procedures[i]);
  for (int32_t i = 0; i < file->checks.count; ++i)
    if (check_mask[(size_t)i]) plan.checks.push_back({i, file->checks[i]});
  std::string emitted = emit_native_checks_c_plan(file, opts, plan);
  *text = std::move(emitted);
  return true;
#endif
}

}  // namespace coax
