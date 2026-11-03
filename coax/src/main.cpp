// coax0 driver and C++ unity frontend. The build-only stage0 compiles the
// Coax compiler modules to C11, which are linked into the final compiler.
//
//   coax run     file.coax [--release]               compile and run
//   coax build   file.coax [-o out] [--release] [--keep-c]
//   coax lib     file.coax [-o out.o] [--release] [--keep-c]
//                                                    an object file of the #export_c procedures
//   coax header  file.coax [-o out.h]                the C header for those procedures
//   coax c       file.coax [--release] [--lib]       print the generated C
//   coax cuda    file.coax [--release]               CUDA C for NVRTC (the compute subset)
//   coax metal-compute file.coax [--release]         Metal compute kernels (the same subset)
//   coax spirv   file.coax [--kernel NAME | --fill A[,B,C,D] | --stroke NAME | --vertex V --fragment F [--tree]] [-o out.spv]
//                                                    SPIR-V for Vulkan (the Metal scalar profile)
//   coax check   file.coax                           check without invoking C
//   coax layout  file.coax                           struct and enum layouts
//   coax test    file.coax [--steps N] [--only S] [--list]  run every #test and #forall (or those named with S; --list names them)
//   coax mutate  file.coax [--jobs N] [--shard I/N] [--merge RECORD]...
//                                                    mutate the code; every mutant must fail a check
//   coax explain CODE | --all [--markdown]           what an error code means
//   coax version
//
// --json prints diagnostics as one JSON object per line on stderr.
// The C compiler is $COAX_CC, or cc.
#include "coax.h"
#ifdef __APPLE__
#include <mach-o/dyld.h>
#endif
#include "release.h"

#ifdef COAX_BOOTSTRAP_IMPORTER
#include "budget.h"
#include "profiles.h"
#include "check-domains.h"
#else
#include "proto_importer.h"
#endif

#include <chrono>
#include <map>

namespace coax {
// Host phase timings: --timings FILE (or --timings-json FILE) writes JSON,
// --timings-table prints a table on stderr. Monotonic clock observations are
// host effects, never interpreted #run work. With neither flag, on() is false
// and no phase reads the clock.
struct TimedCheck {std::string kind,name,phase;double seconds;uint64_t steps,cases;bool passed;};
struct TimedLibrary {std::string path;double seconds;bool cached,checks;};
struct NativeTimings {
  std::string path,source_map;
  bool table=false;
  std::map<std::string,double> seconds,detail;  // detail: parts of a phase (check.checker ...), never summed with seconds
  std::vector<TimedCheck> checks;                // each #test/#forall run (coax test, or an #authority file's checks)
  std::vector<TimedLibrary> libraries;           // each loaded unit's standalone check (cached or run)
  const char *check_phase="test";
  std::string parallel;                          // how the #authority checks ran (parallel_checks), "" when they did not
  size_t generated_bytes=0,instances=0,instance_bytes=0;
  uint64_t run_steps=100000, evaluated_steps=0, peak_slots=0, peak_containers=0, peak_string_bytes=0, traversal_work=0;
  std::chrono::steady_clock::time_point start=std::chrono::steady_clock::now();
  bool on() const {return !path.empty() || table;}
} g_timings;
static double seconds_since(std::chrono::steady_clock::time_point t) {return std::chrono::duration<double>(std::chrono::steady_clock::now()-t).count();}
struct TimedPhase {
  const char *name;
  bool timed=g_timings.on();
  std::chrono::steady_clock::time_point start=timed?std::chrono::steady_clock::now():std::chrono::steady_clock::time_point{};
  ~TimedPhase() {if(timed && g_timings.on())g_timings.seconds[name]+=seconds_since(start);}
};
// Consecutive parts of one phase: lap(name) adds the time since the last lap to detail[name].
struct TimedLaps {
  bool timed=g_timings.on();
  std::chrono::steady_clock::time_point last=timed?std::chrono::steady_clock::now():std::chrono::steady_clock::time_point{};
  void lap(const char *name) {if(!timed)return;auto now=std::chrono::steady_clock::now();g_timings.detail[name]+=std::chrono::duration<double>(now-last).count();last=now;}
};
}  // namespace coax

#include "check.cpp"
#include "errors.cpp"
#include "eval.cpp"
#include "emit_c.cpp"
#include "native_checks.cpp"
#include "effects.cpp"
#include "compute.cpp"
#include "spirv.cpp"
#include "shader.cpp"
#include "lex.cpp"
#include "parse.cpp"
#include "verify.cpp"
#include "proto.cpp"
#include "sources.cpp"

#include <filesystem>
#include <cerrno>
#include <chrono>
#include <map>
#include <sstream>
#include <iomanip>
#include <set>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>
#include <fcntl.h>

extern char **environ;

namespace coax {

static const char *VERSION = COAX_RELEASE_VERSION;

int32_t g_error_count = 0;
bool g_json_errors = false;
bool g_diagnostics_json = false;

void *arena_alloc(Arena *a, size_t size) {
  size = (size + 15) & ~(size_t)15;
  if (a->used + size > a->cap) {
    size_t cap = size > (1u << 20) ? size : (1u << 20);
    a->block = (uint8_t *)malloc(cap);  // the old block stays alive: nodes point into it
    if (!a->block) { fprintf(stderr, "coax: out of memory\n"); exit(2); }
    a->cap = cap;
    a->used = 0;
  }
  void *p = a->block + a->used;
  a->used += size;
  return p;
}

std::string json_string(const std::string &s) {
  std::string out = "\"";
  for (unsigned char c : s) {
    if (c == '"' || c == '\\') { out += '\\'; out += (char)c; }
    else if (c == '\n') out += "\\n";
    else if (c < 0x20) {
      char buf[8];
      snprintf(buf, sizeof buf, "\\u%04x", c);
      out += buf;
    } else out += (char)c;
  }
  return out + "\"";
}

Loc diagnostic_loc(const Source *src, Loc loc) {
  for (const GeneratedRegion &region : src->generated)
    if (loc.offset >= region.start && loc.offset < region.end) return region.directive;
  return loc;
}

// --diagnostics json (schema coax.diagnostic.v1). A span is a file, its
// 1-based line and column, a byte offset into that file and a length: the
// token that starts there (a name, a number or a string), one byte, or zero at
// a line's end. A fix whose text is "add 'X'" or "add 'X' before the end of the
// line" also carries the insertion as an edit; other fixes carry text only.
struct PendingNote { const Source *src; Loc loc; bool has_loc; std::string message; };
static std::vector<PendingNote> g_pending_notes;

void diagnostic_note(const Source *src, Loc loc, bool has_loc, const std::string &message) {
  if (g_diagnostics_json) g_pending_notes.push_back({src, loc, has_loc, message});
}

struct FileSpan { std::string path; int32_t line = 0, col = 0, offset = 0, length = 0; const std::string *text = nullptr; };

static FileSpan file_span(const Source *src, Loc loc) {
  Loc joined = diagnostic_loc(src, loc);
  auto point = source_point(src, joined);
  FileSpan span;
  span.path = point.path;
  span.line = point.loc.line;
  span.col = point.loc.col;
  const std::string *text = &(src->original_text.empty() ? src->text : src->original_text);
  int32_t offset = joined.offset;
  for (const auto &unit : src->units)
    if (offset >= unit->begin && offset < unit->end) { text = &unit->text; offset -= unit->begin; break; }
  if (offset < 0) offset = 0;
  if ((size_t)offset > text->size()) offset = (int32_t)text->size();
  span.offset = offset;
  span.text = text;
  const std::string &t = *text;
  size_t at = (size_t)offset, end = at;
  auto word = [](unsigned char c) { return c == '_' || isalnum(c); };
  if (at >= t.size() || t[at] == '\n') end = at;
  else if (word((unsigned char)t[at])) { while (end < t.size() && word((unsigned char)t[end])) end++; }
  else if (t[at] == '"') { end = at + 1; while (end < t.size() && t[end] != '"' && t[end] != '\n') end += t[end] == '\\' && end + 1 < t.size() ? 2 : 1; if (end < t.size() && t[end] == '"') end++; }
  else end = at + 1;
  span.length = (int32_t)(end - at);
  return span;
}

static std::string span_json(const FileSpan &span) {
  return "{\"file\":" + json_string(span.path) + ",\"line\":" + std::to_string(span.line) + ",\"column\":" + std::to_string(span.col) +
         ",\"offset\":" + std::to_string(span.offset) + ",\"length\":" + std::to_string(span.length) + "}";
}

static std::string fix_edits(const FileSpan &at, const std::string &fix) {
  if (fix.size() < 7 || fix.compare(0, 5, "add '") != 0) return "[]";
  size_t close = fix.find('\'', 6);
  if (close == std::string::npos) return "[]";
  std::string insert = fix.substr(5, close - 5), rest = fix.substr(close + 1);
  FileSpan edit = at;
  edit.length = 0;
  if (rest == " before the end of the line") {
    size_t end = (size_t)at.offset;
    while (end < at.text->size() && (*at.text)[end] != '\n') end++;
    edit.col += (int32_t)(end - (size_t)at.offset);
    edit.offset = (int32_t)end;
  } else if (!rest.empty()) return "[]";
  return "[{\"span\":" + span_json(edit) + ",\"replacement\":" + json_string(insert) + "}]";
}

static void diagnostic_json(const char *severity, const char *code, const std::string &message, const std::string &span,
                            const std::string &notes, const std::string &fixes) {
  fprintf(stderr, "{\"schema\":\"coax.diagnostic.v1\",\"severity\":\"%s\",\"code\":%s,\"message\":%s,\"span\":%s,\"notes\":[%s],\"fixes\":%s}\n",
          severity, json_string(code).c_str(), json_string(message).c_str(), span.c_str(), notes.c_str(), fixes.c_str());
}

static bool g_quiet_errors = false;  // a library's standalone check counts its errors and prints none
void error_at(const Source *src, Loc loc, const char *code, const std::string &message, const std::string &fix) {
  g_error_count++;
  if (g_quiet_errors) { g_pending_notes.clear(); return; }
  std::string reported = message;
  for (const GeneratedRegion &region : src->generated)
    if (loc.offset >= region.start && loc.offset < region.end)
      reported += " (generated proto import " + region.prefix + ")";
  if (g_diagnostics_json) {
    FileSpan span = file_span(src, loc);
    std::string notes;
    for (const PendingNote &note : g_pending_notes)
      notes += (notes.empty() ? "" : ",") + std::string("{\"message\":") + json_string(note.message) + ",\"span\":" +
               (note.has_loc ? span_json(file_span(note.src, note.loc)) : "null") + "}";
    g_pending_notes.clear();
    std::string fixes = fix.empty() ? "[]" : "[{\"message\":" + json_string(fix) + ",\"edits\":" + fix_edits(span, fix) + "}]";
    diagnostic_json("error", code, reported, span_json(span), notes, fixes);
    return;
  }
  Loc joined = diagnostic_loc(src, loc);
  auto point = source_point(src, joined);
  loc = point.loc;
  if (g_json_errors) {
    fprintf(stderr, "{\"file\": %s, \"line\": %d, \"col\": %d, \"code\": \"%s\", \"message\": %s, \"fix\": %s}\n",
            json_string(point.path).c_str(), loc.line, loc.col, code, json_string(reported).c_str(),
            json_string(fix).c_str());
    return;
  }
  fprintf(stderr, "%s:%d:%d: error[%s]: %s\n", point.path.c_str(), loc.line, loc.col, code, reported.c_str());
  show_source(stderr, src, joined);
  if (!fix.empty()) fprintf(stderr, "  fix: %s\n", fix.c_str());
}

void error_after(const Source *src, Loc loc, const char *code, const std::string &message, const std::string &fix,
                 const Source *cause_src, Loc cause) {
  const auto point = source_point(cause_src, cause);
  const std::string note = "this may follow from the error at " + point.path + ":" + std::to_string(point.loc.line) + ":" +
                           std::to_string(point.loc.col) + "; fix that one first";
  diagnostic_note(cause_src, cause, true, note);
  error_at(src, loc, code, message, fix);
  if (!g_quiet_errors && !g_json_errors) fprintf(stderr, "  note: %s\n", note.c_str());
}

bool error_limit() {
  static bool said = false;
  if (g_error_count < PS_report_limit()) return false;
  if (!said && !g_quiet_errors && (g_diagnostics_json || !g_json_errors)) {
    const std::string text = "stopped after " + std::to_string(g_error_count) + " errors; fix these and check again";
    if (g_diagnostics_json) diagnostic_json("note", "", text, "null", "", "[]");
    else fprintf(stderr, "coax: %s\n", text.c_str());
  }
  said = true;
  return true;
}

void show_source(FILE *out, const Source *src, Loc loc) {
  loc = diagnostic_loc(src, loc);
  const std::string *text = &(src->original_text.empty() ? src->text : src->original_text);
  for (const auto &unit:src->units) if (loc.offset>=unit->begin && loc.offset<unit->end) {
    text=&unit->text;loc.offset-=unit->begin;break;
  }
  const std::string &t = *text;
  size_t start = (size_t)loc.offset;
  if (start > t.size()) start = t.size();
  while (start > 0 && t[start - 1] != '\n') start--;
  size_t end = start;
  while (end < t.size() && t[end] != '\n') end++;
  if (end > start) {
    fprintf(out, "  %.*s\n  ", (int)(end - start), t.data() + start);
    for (size_t i = start; i < (size_t)loc.offset && i < end; i++) fputc(t[i] == '\t' ? '\t' : ' ', out);
    fputs("^\n", out);
  }
}


static bool write_file(const std::string &path, const std::string &data) {
  FILE *f = fopen(path.c_str(), "wb");
  if (!f) return false;
  bool ok = fwrite(data.data(), 1, data.size(), f) == data.size();
  return fclose(f) == 0 && ok;
}

// Runs argv[0] from PATH and returns its exit status (128+signal if killed).
static int run_process(const std::vector<std::string> &args,bool *started=nullptr) {
  if(started)*started=false;
  std::vector<char *> argv;
  for (const std::string &a : args) argv.push_back((char *)a.c_str());
  argv.push_back(nullptr);
  pid_t pid;
  int spawn_error = posix_spawnp(&pid, argv[0], nullptr, nullptr, argv.data(), environ);
  if (spawn_error != 0) {
    fprintf(stderr, "coax: cannot start '%s'\n", argv[0]);
    return 127;
  }
  if(started)*started=true;
  int status = 0;
  while (waitpid(pid, &status, 0) < 0) {if (errno != EINTR) return 1;}
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  return 1;
}

static size_t g_library_cached = 0, g_library_checked = 0;  // library checks reused and run (ax-library-check-cache)
static size_t g_authority_cached = 0, g_authority_checked = 0;
static uint64_t g_check_slots = 0;  // --slots N on a compile: a smaller value-slot limit for its checks (0: the test limit)  // #authority check runs reused and run (ax-authority-check-cache)

// Observations of the checked Coax selection and host I/O. Hashing uses the
// existing Coax SHA-256 implementation, never a native fallback.
struct SemanticReport {
  std::string path,command,backend,emitted,artifact;
  coax_t_CP_Profile selection{};
  bool resolved=false,native_invoked=false,inputs_admitted=false;
  std::vector<std::pair<std::string,std::string>> sources,descriptors;
} g_semantic;
static bool profile_destination_disjoint(const std::string &path) {
  if(g_semantic.path.empty() || path.empty())return true;
  std::error_code first_error,second_error;
  auto report=std::filesystem::weakly_canonical(g_semantic.path,first_error);
  auto other=std::filesystem::weakly_canonical(path,second_error);
  bool same=!first_error && !second_error && report==other;
  std::error_code alias_error;
  same=same || std::filesystem::equivalent(g_semantic.path,path,alias_error);
  if(!same)return true;
  fprintf(stderr,"coax: profile report '%s' aliases an input or output '%s'\n",g_semantic.path.c_str(),path.c_str());
  g_semantic.resolved=false;g_timings.path.clear();return false;
}
static std::string profile_file_hash(const std::string &path) {
  std::string bytes;
  if(!read_record(path,&bytes))return "";
  return debug_source_hash(bytes);
}
static int finish_semantic_report(int status) {
  if(g_semantic.path.empty() || !g_semantic.resolved || !g_semantic.inputs_admitted)return status;
  const auto &p=g_semantic.selection;
  auto nullable=[](const std::string &s){return s.empty()?std::string("null"):json_string(s);};
  auto inputs=[](const auto &rows){
    std::string out="[";bool first=true;
    for(const auto &[path,hash]:rows){out+=(first?"":",")+std::string("{\"path\":")+json_string(path)+",\"sha256\":"+json_string(hash)+"}";first=false;}
    return out+"]";
  };
  if(!p.native && status==0)g_semantic.artifact=g_semantic.emitted;
  std::string text="{\"schema_version\":1,\"compiler\":"+json_string(VERSION)+",\"command\":"+json_string(g_semantic.command)+
    ",\"backend\":"+json_string(g_semantic.backend)+",\"profile_identity\":"+std::to_string(p.identity)+
    ",\"runtime_arithmetic\":"+json_string(p.arithmetic==1?"wrapping":"checked")+",\"interpreter_arithmetic\":\"checked\",\"build_profile\":"+
    json_string(p.development?"development":"release")+",\"selected_optimization_level\":"+std::to_string(p.optimization)+
    ",\"native_optimization_flag\":"+(g_semantic.native_invoked?json_string("-O"+std::to_string(p.optimization)):"null")+
    ",\"debug_info\":"+(p.debug_info?"true":"false")+",\"success\":"+(status==0?"true":"false")+",\"exit_status\":"+std::to_string(status)+
    ",\"sources\":"+inputs(g_semantic.sources)+",\"descriptors\":"+inputs(g_semantic.descriptors)+",\"emitted_sha256\":"+nullable(g_semantic.emitted)+
    ",\"artifact_sha256\":"+nullable(g_semantic.artifact)+"}\n";
  if(!write_file(g_semantic.path,text)){fprintf(stderr,"coax: cannot write profile report '%s'\n",g_semantic.path.c_str());return 1;}
  return status;
}
// The phases in pipeline order; a phase not named here follows them.
static const char *const TIMED_PHASES[]={"load","lex","imports","libraries","parse","check","evaluate","authority","emit","native_compile","link"};
static std::string timed_path(const std::string &path) {
  std::error_code ec;
  const auto rel=std::filesystem::path(path).lexically_relative(std::filesystem::current_path(ec));
  return ec || rel.empty() || *rel.begin()==".." ? path : rel.string();
}
static void timings_table(double total) {
  std::vector<std::string> order(std::begin(TIMED_PHASES),std::end(TIMED_PHASES));
  for(const auto &[phase,duration]:g_timings.seconds)if(std::find(order.begin(),order.end(),phase)==order.end())order.push_back(phase);
  const double all=total>0?total:1;
  fprintf(stderr,"coax timings: %.3f s wall\n  %-26s %10s %7s\n",total,"phase","seconds","share");
  for(const std::string &phase:order){
    auto at=g_timings.seconds.find(phase);if(at==g_timings.seconds.end())continue;
    fprintf(stderr,"  %-26s %10.3f %6.1f%%\n",phase.c_str(),at->second,100*at->second/all);
    for(const auto &[part,duration]:g_timings.detail)
      if(part.size()>phase.size() && part.compare(0,phase.size()+1,phase+".")==0)fprintf(stderr,"    %-24s %10.3f %6.1f%%\n",part.c_str(),duration,100*duration/all);
  }
  auto slowest=[](auto rows,auto print){
    std::sort(rows.begin(),rows.end(),[](const auto &a,const auto &b){return a.seconds>b.seconds;});
    for(size_t i=0;i<rows.size() && i<10;i++)print(rows[i]);
    if(rows.size()>10)fprintf(stderr,"    ... %zu more\n",rows.size()-10);
  };
  if(!g_timings.checks.empty()){
    double sum=0;for(const auto &c:g_timings.checks)sum+=c.seconds;
    fprintf(stderr,"  checks: %zu run, %.3f s; slowest:\n",g_timings.checks.size(),sum);
    slowest(g_timings.checks,[](const TimedCheck &c){fprintf(stderr,"    %9.3f s %12llu steps  %s %s%s (%s)\n",c.seconds,(unsigned long long)c.steps,c.kind.c_str(),c.name.c_str(),c.passed?"":" FAILED",c.phase.c_str());});
  }
  if(g_authority_cached)fprintf(stderr,"  authority: every check passed in an earlier compile of these sources; reused, none run\n");
  else if(!g_timings.parallel.empty())fprintf(stderr,"  authority checks: %s\n",g_timings.parallel.c_str());
  if(!g_timings.libraries.empty()){
    double ran=0,cached=0;size_t n_ran=0;
    for(const auto &l:g_timings.libraries){if(l.cached)cached+=l.seconds;else {ran+=l.seconds;n_ran++;}}
    fprintf(stderr,"  libraries: %zu checked alone (%.3f s), %zu cached (%.3f s); slowest:\n",n_ran,ran,g_timings.libraries.size()-n_ran,cached);
    slowest(g_timings.libraries,[](const TimedLibrary &l){fprintf(stderr,"    %9.3f s %-7s %s\n",l.seconds,l.cached?"cached":"checked",timed_path(l.path).c_str());});
  }
}
static int finish_profile(int status) {
  status=finish_semantic_report(status);
  if(!g_timings.on()) return status;
  const double total=seconds_since(g_timings.start);
  if(g_timings.table)timings_table(total);
  if(g_timings.path.empty()) return status;
  std::ostringstream out;out<<std::setprecision(17)<<"{\"schema_version\":1,\"compiler\":"<<json_string(VERSION)<<",\"success\":"<<(status==0?"true":"false")<<",\"exit_status\":"<<status<<",\"generated_bytes\":"<<g_timings.generated_bytes<<",\"instances\":"<<g_timings.instances<<",\"instance_generated_bytes\":"<<g_timings.instance_bytes<<",\"run_steps\":"<<g_timings.run_steps<<",\"evaluated_steps\":"<<g_timings.evaluated_steps<<",\"aggregate_peak_slots\":"<<g_timings.peak_slots<<",\"aggregate_peak_containers\":"<<g_timings.peak_containers<<",\"string_peak_bytes\":"<<g_timings.peak_string_bytes<<",\"aggregate_traversal_work\":"<<g_timings.traversal_work<<",\"library_checks\":{\"cached\":"<<g_library_cached<<",\"checked\":"<<g_library_checked<<"}"<<",\"authority_checks\":{\"cached\":"<<g_authority_cached<<",\"checked\":"<<g_authority_checked<<"}"<<",\"parallel_checks\":"<<json_string(g_timings.parallel)<<",\"seconds\":{";
  bool first=true;for(const auto &[phase,duration]:g_timings.seconds) {out<<(first?"":",")<<json_string(phase)<<":"<<duration;first=false;}
  out<<(first?"":",")<<"\"total\":"<<total<<"},\"detail_seconds\":{";
  first=true;for(const auto &[part,duration]:g_timings.detail) {out<<(first?"":",")<<json_string(part)<<":"<<duration;first=false;}
  out<<"},\"checks\":[";
  first=true;for(const auto &c:g_timings.checks) {out<<(first?"":",")<<"{\"kind\":"<<json_string(c.kind)<<",\"name\":"<<json_string(c.name)<<",\"phase\":"<<json_string(c.phase)<<",\"seconds\":"<<c.seconds<<",\"steps\":"<<c.steps<<",\"cases\":"<<c.cases<<",\"passed\":"<<(c.passed?"true":"false")<<"}";first=false;}
  out<<"],\"libraries\":[";
  first=true;for(const auto &l:g_timings.libraries) {out<<(first?"":",")<<"{\"path\":"<<json_string(timed_path(l.path))<<",\"seconds\":"<<l.seconds<<",\"cached\":"<<(l.cached?"true":"false")<<",\"checks_alone\":"<<(l.checks?"true":"false")<<"}";first=false;}
  out<<"]}\n";
  if(!write_file(g_timings.path,out.str())) {fprintf(stderr,"coax: cannot write timing report '%s'\n",g_timings.path.c_str());return 1;}return status;
}

// The coax.evidence.v1 record (see Evidence in verify.cpp). Every claim is
// bound to the source units, compiler, evaluator target, profile, selection
// and budgets; the verdict is EV_verdict over the claims' outcomes, so a run
// that executed nothing, or stopped early, is never a pass.
static int write_evidence(const std::string &cmd, const std::string &input, const Source *src, const EmitOptions &opts, bool checked,
                          int status, uint64_t steps, uint64_t slots, const char *only, bool list, const MutationOptions &mutation) {
  if (g_evidence.path.empty()) return status;
  static const char *names[] = {"passed", "counterexample", "inconclusive", "infrastructure-error", "not-checked"};
  auto outcome = [](uint32_t o) { return json_string(names[o < 5 ? o : 3]); };
  uint32_t bits = 0;
  std::string claims;
  for (const EvidenceClaim &c : g_evidence.claims) {
    bits = EV_add(bits, c.outcome);
    std::string at = "null";
    if (c.loc.line) {
      auto p = source_point(src, c.loc);
      at = "{\"file\":" + json_string(p.path) + ",\"line\":" + std::to_string(p.loc.line) + ",\"col\":" + std::to_string(p.loc.col) + "}";
    }
    claims += std::string(claims.empty() ? "" : ",") + "{\"kind\":" + json_string(c.kind) + ",\"name\":" + json_string(c.name) +
              ",\"outcome\":" + outcome(c.outcome) + ",\"at\":" + at + ",\"cases\":" + std::to_string(c.cases) +
              ",\"steps\":" + std::to_string(c.steps) + (c.code.empty() ? "" : ",\"code\":" + json_string(c.code)) +
              (c.message.empty() ? "" : ",\"message\":" + json_string(c.message)) +
              (c.counterexample.empty() ? "" : ",\"counterexample\":" + json_string(c.counterexample)) + "}";
  }
  std::string mutation_row = "null";
  if (cmd == "mutate") {
    uint32_t m = g_evidence.infrastructure ? 3 : g_evidence.mutation && !g_evidence.mutation_detail.empty() && checked ? g_evidence.mutation_outcome : 4;
    if (g_evidence.mutation_detail.rfind("\"reason\"", 0) == 0) m = 4;
    bits = EV_add(bits, m);
    mutation_row = "{\"outcome\":" + outcome(m) + (g_evidence.mutation_detail.empty() ? "" : "," + g_evidence.mutation_detail) + "}";
  }
  if (g_evidence.infrastructure) bits = EV_add(bits, 3);
  if (!checked) bits = EV_add(bits, 4);
  std::string sources;
  for (const auto &unit : src->units)
    sources += std::string(sources.empty() ? "" : ",") + "{\"path\":" + json_string(unit->canonical) + ",\"sha256\":" + json_string(debug_source_hash(unit->text)) + "}";
  std::string compiler_hash;
  {
    char self[4096];
    uint32_t size = sizeof self;
#ifdef __APPLE__
    if (_NSGetExecutablePath(self, &size) == 0) compiler_hash = profile_file_hash(self);
#else
    ssize_t n = readlink("/proc/self/exe", self, sizeof self - 1);
    if (n > 0) { self[n] = 0; compiler_hash = profile_file_hash(self); }
#endif
    (void)size;
  }
  std::string text = "{\"schema\":\"coax.evidence.v1\",\"command\":" + json_string(cmd) + ",\"input\":" + json_string(input) +
      ",\"compiler\":{\"version\":" + json_string(VERSION) + ",\"sha256\":" + (compiler_hash.empty() ? "null" : json_string(compiler_hash)) + "}" +
      ",\"target\":{\"name\":\"coax-evaluator\",\"note\":\"checks run in the compile-time evaluator, not generated code\"}" +
      ",\"profile\":{\"arithmetic\":\"checked\",\"selected_runtime_arithmetic\":" + json_string(opts.wrapping ? "wrapping" : "checked") + ",\"library\":" + (opts.library ? "true" : "false") + "}" +
      ",\"selection\":{\"only\":" + (only ? json_string(only) : "null") + ",\"list\":" + (list ? "true" : "false") +
      ",\"root_only\":" + (mutation.root_only ? "true" : "false") + "}" +
      ",\"budgets\":{\"check_steps\":" + std::to_string(cmd == "mutate" ? TEST_STEPS : steps) + ",\"check_slots\":" + std::to_string(slots) +
      ",\"check_containers\":" + std::to_string(CB_test_container_limit()) + ",\"run_steps\":" + std::to_string(opts.run_steps) + "}" +
      ",\"sources\":[" + sources + "],\"front_end\":" + (checked ? "\"ok\"" : "\"failed\"") +
      ",\"claims\":[" + claims + "],\"mutation\":" + mutation_row + ",\"exit_status\":" + std::to_string(status) +
      ",\"verdict\":" + outcome(EV_verdict(bits)) + "}\n";
  if (!write_file(g_evidence.path, text)) {
    fprintf(stderr, "coax: cannot write evidence record '%s'\n", g_evidence.path.c_str());
    return 1;
  }
  return status;
}

// Descriptor parsing and code generation run inside the compiler.
// Generated declarations are appended after user text to preserve user locations.
static bool import_protos(Source *src, const EmitOptions &opts, std::vector<Token> &tokens) {
  struct Request { std::string path, prefix; Loc loc; int end; };
  std::vector<Request> requests;
  auto input=parser_tokens(tokens);
  const auto caps=PS_caps((int64_t)input.size(),(int64_t)src->text.size());
  std::vector<coax_t_PS_Import> records((size_t)caps.imports);
  const auto scan=PS_imports({input.data(),(int64_t)input.size()},{records.data(),(int64_t)records.size()},opts.metal);
  if(scan.code.count) {
    error_at(src,parser_loc(scan.loc),std::string(scan.code.data,(size_t)scan.code.count).c_str(),
        std::string(scan.message.data,(size_t)scan.message.count));return false;
  }
  if(!scan.count) return true;
  for(int64_t i=0;i<scan.count;i++) {
    const auto &r=records[(size_t)i];requests.push_back({to_std(parser_str(r.path)),to_std(parser_str(r.prefix)),parser_loc(r.loc),(int)r.end});
  }
  src->original_text = src->text;
  std::string generated;
  for (const Request &r : requests) {
    std::filesystem::path path(r.path);
    if (path.is_relative()) path = std::filesystem::absolute(std::filesystem::path(source_point(src, r.loc).path).parent_path() / path);
    src->descriptor_inputs.push_back(std::filesystem::absolute(path).lexically_normal().string());
    if(!profile_destination_disjoint(src->descriptor_inputs.back()))return false;
    proto::Result result = proto::import_file(path.lexically_normal().string(), r.prefix, VERSION,!g_semantic.path.empty());
    if (!result.error.empty()) {
      error_at(src, r.loc, "E0502", result.error);
      return false;
    }
    if(!g_semantic.path.empty())g_semantic.descriptors.emplace_back(src->descriptor_inputs.back(),result.descriptor_sha256);
    std::string &text = result.source;
    if (generated.size() + text.size() + 1 > 16 * 1024 * 1024) {
      error_at(src, r.loc, "E0502", "generated import exceeds source budget");
      return false;
    }
    src->import_provenance.push_back(result.provenance);
    int32_t start = (int32_t)(src->text.size() + generated.size() + 1);
    src->generated.push_back({start, start + (int32_t)text.size(), r.loc, r.prefix});
    generated += "\n" + text;
  }
  for (const Request &r : requests)
    for (int i = r.loc.offset; i < r.end; i++) if (src->text[(size_t)i] != '\n') src->text[(size_t)i] = ' ';
  src->text += generated;
  return true;
}

enum class Output : uint8_t { none, c, header, layout, deps, declarations, modules, lowering, effects };

// `coax deps --declarations` (schema coax.declarations.v1): each unit's
// logical owner, every top-level declaration's nominal ID and every resolved
// reference between declarations, from the checked graph. IDs come from the
// checked tuple encoding (lib/compiler/sources.coax); null means the
// declaring unit has no #module owner, so no stable identity is claimed.
struct DeclarationReport {
  const Source *src;
  std::string text;
  bool first = true;
  std::string id_json(uint8_t kind, Str name, Loc loc) const {
    std::string id = declaration_id(src, kind, name, loc);
    return id.empty() ? "null" : json_string(id);
  }
  std::string unit_json(Loc loc) const {
    const SourceUnit *unit = source_unit_at(src, loc);
    return unit ? json_string(unit->canonical) : "null";
  }
  void reference(const std::string &from, uint8_t kind, Str name, Loc decl, Loc at) {
    static const char *KINDS[] = {"proc", "struct", "enum", "const"};
    text += std::string(first ? "" : ",") + "{\"from\":" + from + ",\"to\":" + id_json(kind, name, decl) +
            ",\"kind\":\"" + KINDS[kind] + "\",\"name\":" + json_string(to_std(name)) +
            ",\"unit\":" + unit_json(at) + ",\"line\":" + std::to_string(source_point(src, at).loc.line) + "}";
    first = false;
  }
  void type(const std::string &from, Type t, Loc at) {
    while (t && (t->kind == TypeKind::array || t->kind == TypeKind::view)) t = t->elem;
    if (!t) return;
    if (t->kind == TypeKind::struct_ && t->struct_decl) reference(from, 1, t->struct_decl->name, t->struct_decl->loc, at);
    if (t->kind == TypeKind::enum_ && t->enum_decl) reference(from, 2, t->enum_decl->name, t->enum_decl->loc, at);
  }
  void expr(const std::string &from, const Expr *e) {
    if (!e) return;
    if (e->kind == ExprKind::call && e->proc) reference(from, 0, e->proc->name, e->proc->loc, e->loc);
    if (e->kind == ExprKind::ident && e->constant) reference(from, 3, e->constant->name, e->constant->loc, e->loc);
    if (e->kind == ExprKind::struct_lit || e->kind == ExprKind::cast || e->kind == ExprKind::enum_value || e->kind == ExprKind::array_lit) type(from, e->type, e->loc);
    expr(from, e->lhs); expr(from, e->rhs); expr(from, e->third);
    for (const Expr *a : e->args) expr(from, a);
    for (const FieldInit &f : e->fields) expr(from, f.value);
  }
  void stmt(const std::string &from, const Stmt *s) {
    if (!s) return;
    if (s->kind == StmtKind::var || s->kind == StmtKind::for_range || s->kind == StmtKind::for_each) type(from, s->type, s->loc);
    expr(from, s->expr); expr(from, s->expr2); expr(from, s->target);
    for (const Stmt *b : s->body) stmt(from, b);
    stmt(from, s->then_); stmt(from, s->else_);
  }
};

// `coax deps --modules` (schema coax.modules.v1): conservative module-scoped
// revisions, separate from nominal IDs, physical source hashes and the
// selected profile. A module's content revision covers its member units; its
// revision adds what it depends on: other modules' revisions, the hashes of
// unowned units it loads (transitively) and, when a member imports a
// protobuf descriptor, every descriptor input. Canonical revision text and
// digest order are checked in lib/compiler/sources.coax (SU_revision_text).
static std::string read_bytes(const std::string &path) {
  FILE *f = fopen(path.c_str(), "rb");
  if (!f) return "";
  std::string out; char buffer[65536]; size_t n;
  while ((n = fread(buffer, 1, sizeof buffer, f)) > 0) out.append(buffer, n);
  fclose(f);
  return out;
}
// A declaration's exact authored range: unit-relative byte offsets of its
// first and one-past-last token, their lines and the sha256 of those bytes.
// A generated declaration has no authored range; it names its directive.
static std::string span_json(const Source *src, Loc loc, int32_t end) {
  for (const GeneratedRegion &region : src->generated)
    if (loc.offset >= region.start && loc.offset < region.end) {
      const SourceUnit *u = source_unit_at(src, region.directive);
      return "\"span\":null,\"origin\":{\"directive\":\"#import_proto\",\"unit\":" + (u ? json_string(u->canonical) : std::string("null")) +
             ",\"line\":" + std::to_string(source_point(src, region.directive).loc.line) + "}";
    }
  const SourceUnit *u = source_unit_at(src, loc);
  if (!u || end <= loc.offset || loc.offset < u->begin || end > u->end || (size_t)(end - u->begin) > u->text.size())
    return "\"span\":null,\"origin\":{\"unavailable\":\"no exact authored range is recorded for this declaration\"}";
  const int32_t start = loc.offset - u->begin, stop = end - u->begin;
  int end_line = 1;
  for (int32_t i = 0; i < stop; i++) end_line += u->text[(size_t)i] == '\n';
  const std::string bytes = u->text.substr((size_t)start, (size_t)(stop - start));
  return "\"span\":{\"start\":" + std::to_string(start) + ",\"end\":" + std::to_string(stop) + ",\"start_line\":" +
         std::to_string(source_point(src, loc).loc.line) + ",\"end_line\":" + std::to_string(end_line) +
         ",\"sha256\":" + json_string(debug_source_hash(bytes)) + "},\"origin\":\"authored\"";
}
static std::string g_target = "native-lp64";
static std::string target_json() {
  const auto t = CK_target({g_target.data(), (int64_t)g_target.size()});
  auto text = [](coax_string v) { return json_string(std::string(v.data, (size_t)v.count)); };
  return "\"target\":{\"name\":" + text(t.name) + ",\"data_model\":" + text(t.data_model) + ",\"pointer\":{\"size\":" + std::to_string(t.pointer_size) +
         ",\"align\":" + std::to_string(t.pointer_alignment) + "},\"count\":{\"size\":" + std::to_string(t.count_size) + ",\"align\":" +
         std::to_string(t.count_alignment) + "},\"view\":{\"size\":" + std::to_string(t.view_size) + ",\"align\":" + std::to_string(t.view_alignment) +
         "},\"scalars\":[" + [] {
           std::string rows;
           for (int id : {2, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15}) {
             const auto b = CK_builtin(id);
             rows += std::string(rows.empty() ? "" : ",") + "{\"type\":" + json_string(std::string(b.name.data, (size_t)b.name.count)) +
                     ",\"size\":" + std::to_string(b.size) + ",\"align\":" + std::to_string(b.alignment) + "}";
           }
           return rows;
         }() + "],\"calling_convention\":" + text(t.calling_convention) + "}";
}
static std::string modules_report(const Source *src, const File *file, const EmitOptions &) {
  const size_t n = src->units.size();
  std::vector<std::string> hash(n);
  for (size_t i = 0; i < n; i++) hash[i] = debug_source_hash(src->units[i]->text);
  auto revision_of = [&](uint8_t kind, std::vector<std::string> own, std::vector<std::string> deps) {
    std::sort(own.begin(), own.end()); std::sort(deps.begin(), deps.end());
    std::vector<coax_string> a, b;
    for (const auto &d : own) a.push_back({d.data(), (int64_t)d.size()});
    for (const auto &d : deps) b.push_back({d.data(), (int64_t)d.size()});
    std::vector<uint8_t> out(64 + 65 * (own.size() + deps.size()));
    int64_t len = SU_revision_text(kind, {a.data(), (int64_t)a.size()}, {b.data(), (int64_t)b.size()}, {out.data(), (int64_t)out.size()});
    return len < 0 ? std::string() : debug_source_hash(std::string((const char *)out.data(), (size_t)len));
  };
  std::vector<std::string> descriptor_hashes;
  for (const auto &path : src->descriptor_inputs) descriptor_hashes.push_back(debug_source_hash(read_bytes(path)));
  std::map<std::string, size_t> owner_unit;
  for (size_t i = 0; i < n; i++) if (src->units[i]->claims) owner_unit[src->units[i]->module] = i;
  std::map<std::string, bool> generated_in;
  for (const GeneratedRegion &region : src->generated) {
    const SourceUnit *u = source_unit_at(src, region.directive);
    if (u && !u->module.empty()) generated_in[u->module] = true;
  }
  std::map<std::string, std::string> content, revision;
  std::map<std::string, std::vector<std::string>> dep_json;
  std::map<std::string, int> state;
  std::function<std::string(const std::string &)> module_revision = [&](const std::string &m) -> std::string {
    if (state[m] == 2) return revision[m];
    if (state[m] == 1) return "";  // unreachable: a module graph cannot cycle (E0602, E0606, E0607)
    state[m] = 1;
    std::vector<std::string> own, deps, rows;
    std::vector<bool> seen(n, false);
    std::function<void(size_t)> walk = [&](size_t u) {
      for (int32_t c : src->units[u]->loads) {
        const SourceUnit *child = src->units[(size_t)c].get();
        if (child->module == m || seen[(size_t)c]) continue;
        seen[(size_t)c] = true;
        if (!child->module.empty()) {
          std::string r = module_revision(child->module);
          deps.push_back(r);
          rows.push_back("{\"module\":" + json_string(child->module) + ",\"revision\":" + json_string(r) + "}");
        } else {
          deps.push_back(hash[(size_t)c]);
          rows.push_back("{\"unit\":" + json_string(child->canonical) + ",\"sha256\":" + json_string(hash[(size_t)c]) + "}");
          walk((size_t)c);
        }
      }
    };
    for (size_t i = 0; i < n; i++) if (src->units[i]->module == m) { own.push_back(hash[i]); walk(i); }
    if (generated_in[m]) for (size_t i = 0; i < descriptor_hashes.size(); i++) {
      deps.push_back(descriptor_hashes[i]);
      rows.push_back("{\"descriptor\":" + json_string(src->descriptor_inputs[i]) + ",\"sha256\":" + json_string(descriptor_hashes[i]) + "}");
    }
    content[m] = revision_of(0, own, {});
    revision[m] = revision_of(1, {content[m]}, deps);
    std::sort(rows.begin(), rows.end());
    dep_json[m] = rows;
    state[m] = 2;
    return revision[m];
  };
  std::string out = "{\"schema\":\"coax.modules.v1\",\"profile\":{\"execution_profile\":" + json_string(file->execution_profile) +
                    ",\"run_steps\":" + std::to_string(file->run_steps) + "},\"modules\":[";
  bool first = true;
  for (const auto &entry : owner_unit) {
    const std::string &m = entry.first;
    module_revision(m);
    out += std::string(first ? "" : ",") + "{\"id\":" + json_string(m) + ",\"owner_unit\":" + json_string(src->units[entry.second]->canonical) + ",\"members\":[";
    first = false;
    bool f2 = true;
    for (size_t i = 0; i < n; i++) if (src->units[i]->module == m) {
      out += std::string(f2 ? "" : ",") + "{\"path\":" + json_string(src->units[i]->canonical) + ",\"sha256\":" + json_string(hash[i]) + "}";
      f2 = false;
    }
    // A sealed module's public API: its #export names, sorted; null when every
    // declaration stays visible (no list).
    std::vector<std::string> names;
    for (size_t i = 0; i < n; i++) if (src->units[i]->module == m) for (const auto &e : src->units[i]->exports) names.push_back(e.name);
    std::sort(names.begin(), names.end());
    std::string exports = names.empty() ? "null" : "[";
    for (size_t i = 0; i < names.size(); i++) exports += (i ? "," : "") + json_string(names[i]);
    if (!names.empty()) exports += "]";
    out += "],\"exports\":" + exports + ",\"content_revision\":" + json_string(content[m]) + ",\"dependencies\":[";
    f2 = true;
    for (const auto &row : dep_json[m]) { out += std::string(f2 ? "" : ",") + row; f2 = false; }
    out += "],\"revision\":" + json_string(revision[m]) + "}";
  }
  out += "],\"declarations\":[";
  std::string legacy, generated;
  first = true;
  auto decl = [&](uint8_t kind, Str name, Loc loc, int32_t end) {
    static const char *KINDS[] = {"proc", "struct", "enum", "const"};
    const SourceUnit *u = source_unit_at(src, loc);
    bool gen = false;
    for (const GeneratedRegion &region : src->generated) gen = gen || (loc.offset >= region.start && loc.offset < region.end);
    std::string row = "{\"kind\":\"" + std::string(KINDS[kind]) + "\",\"name\":" + json_string(to_std(name)) + "," + span_json(src, loc, end);
    if (gen) generated += std::string(generated.empty() ? "" : ",") + row + "}";
    std::string id = declaration_id(src, kind, name, loc);
    if (id.empty()) { legacy += std::string(legacy.empty() ? "" : ",") + row + ",\"unit\":" + (u ? json_string(u->canonical) : "null") + "}"; return; }
    out += std::string(first ? "" : ",") + "{\"id\":" + json_string(id) + ",\"module\":" + json_string(u->module) +
           ",\"revision\":" + json_string(revision[u->module]) + ",\"generated\":" + (gen ? "true" : "false") + "," + span_json(src, loc, end) + "}";
    first = false;
  };
  for (const StructDecl *d : file->structs) if (!d->origin) decl(1, d->name, d->loc, d->end);
  for (const StructDecl *d : file->generic_structs) decl(1, d->name, d->loc, d->end);
  for (const EnumDecl *d : file->enums) decl(2, d->name, d->loc, d->end);
  for (const ConstDecl *d : file->constants) decl(3, d->name, d->loc, d->end);
  for (const ProcDecl *d : file->procs) if (!d->origin) decl(0, d->name, d->loc, d->end);
  for (const ProcDecl *d : file->generics) decl(0, d->name, d->loc, d->end);
  std::string legacy_units;
  for (size_t i = 0; i < n; i++) if (src->units[i]->module.empty())
    legacy_units += std::string(legacy_units.empty() ? "" : ",") + "{\"path\":" + json_string(src->units[i]->canonical) + ",\"sha256\":" + json_string(hash[i]) + "}";
  return out + "]," + target_json() + ",\"unavailable\":{\"declaration_revisions\":\"revisions are module-scoped; each declaration's span sha256 is its exact authored content\"" +
         ",\"legacy_units\":[" + legacy_units + "],\"legacy_declarations\":[" + legacy + "],\"generated_declarations\":[" + generated + "]}}\n";
}

// `coax deps FILE --lowering` (schema coax.lowering.v1): the ordered lowering
// of each procedure in the eligible subset, read from the same decisions the
// C emitter makes (lowering_sequenced, lowering_place_first,
// lowering_compound_staged over the checked graph's CK_effects). Operations
// are listed in execution order: operands, then the operation and its trap.
static std::string g_lower_proc;
static std::string tok_text(Tok t) {
  auto v = LX_name((uint32_t)t);
  std::string n(v.data, (size_t)v.count);
  return n.size() >= 2 && n.front() == '\'' && n.back() == '\'' ? n.substr(1, n.size() - 2) : n;
}
static bool legacy_sum_procedure(const ProcDecl *proc);
struct LoweringReport {
  const Source *src;
  LoweringPolicy policy;
  std::string ops, unsupported;
  bool first = true;
  std::vector<std::pair<int, int>> defers;  // (block depth, line)
  int depth = 0;
  std::string at(Loc loc) {
    auto p = source_point(src, loc);
    const SourceUnit *u = source_unit_at(src, loc);
    return "\"unit\":" + (u ? json_string(u->canonical) : std::string("null")) + ",\"line\":" + std::to_string(p.loc.line) +
           ",\"col\":" + std::to_string(p.loc.col);
  }
  static std::string effects(uint8_t e, LoweringPolicy p) {
    std::string out = "[";
    if (e & 1) out += "\"call\"";
    if (e & 2) out += std::string(out.size() > 1 ? "," : "") + "\"trap\"";
    if ((e & 4) && !p.wrapping) out += std::string(out.size() > 1 ? "," : "") + "\"overflow\"";
    return out + "]";
  }
  void op(const std::string &what, Loc loc, const std::string &extra = "") {
    ops += std::string(first ? "" : ",") + "{\"op\":" + json_string(what) + "," + at(loc) + extra + "}";
    first = false;
  }
  void cleanup(Loc loc, int down_to) {
    std::string lines;
    for (size_t i = defers.size(); i-- > 0;)
      if (defers[i].first >= down_to) lines += std::string(lines.empty() ? "" : ",") + std::to_string(defers[i].second);
    if (!lines.empty()) op("cleanup", loc, ",\"defers\":[" + lines + "]");
  }
  void expr(const Expr *e) {
    if (!e || unsupported.size()) return;
    if (e->kind == ExprKind::run) { unsupported = "#run expressions are not in the lowering subset"; return; }
    if (e->kind == ExprKind::sum || e->kind == ExprKind::some ||
        e->kind == ExprKind::null_ || e->kind == ExprKind::try_ ||
        e->kind == ExprKind::or_else) {
      unsupported = "tagged values and recoverable flow require a sum-capable lowering profile";
      return;
    }
    std::vector<const Expr *> operands;
    for (const Expr *x : {e->lhs, e->rhs, e->third}) if (x) operands.push_back(x);
    for (const Expr *a : e->args) operands.push_back(a);
    for (const FieldInit &f : e->fields) if (f.value) operands.push_back(f.value);
    for (const Expr *x : operands) expr(x);
    const uint8_t own = e->own_effects;
    std::string name;
    switch (e->kind) {
      case ExprKind::index: name = "index"; break;
      case ExprKind::slice: name = "slice"; break;
      case ExprKind::call: name = e->proc ? "call" : "intrinsic"; break;
      case ExprKind::binary: name = std::string("binary ") + tok_text(e->op); break;
      case ExprKind::unary: name = std::string("unary ") + tok_text(e->op); break;
      case ExprKind::cast: name = "cast"; break;
      case ExprKind::struct_lit: name = "struct literal"; break;
      case ExprKind::array_lit: name = "array literal"; break;
      default: return;
    }
    if (!own && operands.size() < 2) return;
    std::string extra = ",\"effects\":" + effects(own, policy);
    if (operands.size() >= 2) extra += std::string(",\"sequenced\":") + (lowering_sequenced(operands, policy) ? "true" : "false");
    if (e->kind == ExprKind::call && e->proc) {
      std::string id = declaration_id(src, 0, e->proc->name, e->proc->loc);
      extra += ",\"calls\":" + (id.empty() ? json_string(to_std(e->proc->name)) : json_string(id));
    }
    op(name, e->loc, extra);
  }
  void stmt(const Stmt *s) {
    if (!s || unsupported.size()) return;
    switch (s->kind) {
      case StmtKind::block: {
        depth++;
        for (const Stmt *x : s->body) stmt(x);
        // A block that ends in return already ran its cleanup there.
        if (s->body.count == 0 || s->body[s->body.count - 1]->kind != StmtKind::ret) cleanup(s->loc, depth);
        while (!defers.empty() && defers.back().first >= depth) defers.pop_back();
        depth--;
        break;
      }
      case StmtKind::expr: expr(s->expr); break;
      case StmtKind::var: expr(s->expr); op("define", s->loc, ",\"name\":" + json_string(to_std(s->name))); break;
      case StmtKind::assign:
        if (s->op == Tok::equal) {
          bool first_place = lowering_place_first(s->target, s->expr, policy);
          expr(s->target); expr(s->expr);
          op("assign", s->loc, std::string(",\"place_first\":") + (first_place ? "true" : "false"));
        } else {
          bool staged = lowering_compound_staged(s->target, s->expr, policy);
          expr(s->target); expr(s->expr);
          op(std::string("compound ") + tok_text(s->op), s->loc,
             std::string(",\"place_once\":true,\"staged\":") + (staged ? "true" : "false") + ",\"effects\":" +
             effects(s->own_effects, policy));
        }
        break;
      case StmtKind::ret: expr(s->expr); cleanup(s->loc, 1); op("return", s->loc); break;
      case StmtKind::if_: expr(s->expr); op("branch", s->loc); stmt(s->then_); stmt(s->else_); break;
      case StmtKind::while_: expr(s->expr); op("loop", s->loc); stmt(s->then_); break;
      case StmtKind::defer: defers.push_back({depth, source_point(src, s->loc).loc.line}); op("defer", s->loc); break;
      case StmtKind::break_: op("break", s->loc); break;
      case StmtKind::continue_: op("continue", s->loc); break;
      case StmtKind::for_range: case StmtKind::for_each: unsupported = "for loops are not in the lowering subset"; break;
      case StmtKind::assert_: unsupported = "#assert statements are not in the lowering subset"; break;
      case StmtKind::switch_: unsupported = "switch requires a sum-capable lowering profile"; break;
    }
  }
};
static std::string lowering_report(const Source *src, const File *file, const EmitOptions &opts) {
  const LoweringPolicy policy{opts.wrapping, true};
  std::string out = "{\"schema\":\"coax.lowering.v1\"," + target_json() + ",\"arithmetic\":" +
                    json_string(opts.wrapping ? "wrapping" : "checked") + ",\"procedures\":[";
  bool first = true;
  for (const ProcDecl *p : file->procs) {
    if (p->origin) continue;  // instances share their template's name; not reported yet
    if (!g_lower_proc.empty() && to_std(p->name) != g_lower_proc) continue;
    std::string id = declaration_id(src, 0, p->name, p->loc);
    std::string row = "{\"name\":" + json_string(to_std(p->name)) + ",\"id\":" + (id.empty() ? "null" : json_string(id)) + "," +
                      span_json(src, p->loc, p->end);
    if (p->foreign || !p->body) row += ",\"eligible\":false,\"reason\":\"no Coax body\"}";
    else {
      LoweringReport r{src, policy};
      if (legacy_sum_procedure(p)) r.unsupported = "tagged values and exhaustive switch require a sum-capable lowering profile";
      else r.stmt(p->body);
      if (r.unsupported.size()) row += ",\"eligible\":false,\"reason\":" + json_string(r.unsupported) + "}";
      else row += ",\"eligible\":true,\"operations\":[" + r.ops + "]}";
    }
    out += std::string(first ? "" : ",") + row;
    first = false;
  }
  return out + "]}\n";
}

static std::string declarations_report(const Source *src, const File *file) {
  std::string out = "{\"schema\":\"coax.declarations.v1\",\"units\":[";
  bool first = true;
  for (const auto &unit : src->units) {
    out += std::string(first ? "" : ",") + "{\"path\":" + json_string(unit->canonical) + ",\"module\":" +
           (unit->module.empty() ? "null" : json_string(unit->module)) + ",\"claims\":" + (unit->claims ? "true" : "false") + "}";
    first = false;
  }
  DeclarationReport refs{src};
  out += "],\"declarations\":[";
  first = true;
  auto decl = [&](uint8_t kind, Str name, Loc loc, int32_t end) {
    static const char *KINDS[] = {"proc", "struct", "enum", "const"};
    bool generated = false;
    for (const GeneratedRegion &region : src->generated) generated = generated || (loc.offset >= region.start && loc.offset < region.end);
    out += std::string(first ? "" : ",") + "{\"kind\":\"" + KINDS[kind] + "\",\"name\":" + json_string(to_std(name)) +
           ",\"id\":" + refs.id_json(kind, name, loc) + ",\"unit\":" + refs.unit_json(loc) +
           ",\"line\":" + std::to_string(source_point(src, loc).loc.line) + ",\"generated\":" + (generated ? "true" : "false") + "," + span_json(src, loc, end) + "}";
    first = false;
  };
  for (const StructDecl *d : file->structs) if (!d->origin) decl(1, d->name, d->loc, d->end);
  for (const StructDecl *d : file->generic_structs) decl(1, d->name, d->loc, d->end);
  for (const EnumDecl *d : file->enums) decl(2, d->name, d->loc, d->end);
  for (const ConstDecl *d : file->constants) decl(3, d->name, d->loc, d->end);
  for (const ProcDecl *d : file->procs) if (!d->origin) decl(0, d->name, d->loc, d->end);
  for (const ProcDecl *d : file->generics) decl(0, d->name, d->loc, d->end);
  for (const StructDecl *d : file->structs) {
    if (d->origin) continue;  // an instance's uses are its template's
    std::string from = refs.id_json(1, d->name, d->loc);
    for (const Field &f : d->fields) { refs.type(from, f.type, f.loc); refs.expr(from, f.init); }
  }
  for (const EnumDecl *d : file->enums) {
    std::string from = refs.id_json(2, d->name, d->loc);
    for (const EnumMember &m : d->members) refs.expr(from, m.value);
  }
  for (const ConstDecl *d : file->constants) { std::string from = refs.id_json(3, d->name, d->loc); refs.type(from, d->type, d->loc); refs.expr(from, d->expr); }
  for (const ProcDecl *d : file->procs) {
    if (d->origin) continue;  // an instance's uses are its template's
    std::string from = refs.id_json(0, d->name, d->loc);
    for (const Param &p : d->params) refs.type(from, p.type, p.loc);
    refs.type(from, d->ret, d->loc);
    refs.stmt(from, d->body);
  }
  return out + "],\"references\":[" + refs.text + "]," + target_json() + "}\n";
}

// Library checks (ax-library-check-cache). A compile that emits C, a header
// or nothing (coax c, header, build, lib, run, check, metal, cuda) checks each
// loaded unit's #test and #forall bodies in that unit's own #load closure, not
// in every consumer, and records the result. A unit whose checks pass there is
// a library for this compile: its check bodies are left out of the consumer's
// input, so the consumer doesn't check them again or emit the instances and
// types only they reach. A unit that doesn't check on its own (a part that
// uses its loader's names, or a check with an error) keeps today's behavior:
// the consumer checks its bodies in place and reports any error there. The
// root's own checks, and those of a unit whose #load closure has an
// #authority unit or an #import_proto, are never left out. When the program
// has an #authority unit, its compile runs every check, so a library's
// verdict there also requires its checks to pass when run, with the budgets
// the compile would give them.
//
// The verdict for a unit is stored in ~/.cache/coax-check ($COAX_CHECK_CACHE)
// under the SHA-256 of the compiler binary, the options that change checking,
// and each closure unit's path relative to the unit and its SHA-256. A
// standalone check runs in a forked child, so its storage and diagnostics never
// reach the consumer; the child exits 0 when the unit checks. Build-only stage0
// can't hash, so it keeps verdicts only while it runs. Either way the verdict is
// the same, so the emitted C is the same with and without the cache.
static bool g_library_checks = false;     // set by compile() for C, header and check output
static std::map<std::string, bool> g_library_verdicts;  // canonical unit -> checks standalone (this process)
static std::string executable_path();
static bool front_end(const char *path, Source *src, Arena *arena, File *file, const EmitOptions &opts, int library,
                      bool run_checks, bool mutation_lowering, struct QueryParsed *parsed);

static std::string library_cache_dir() {
  const char *dir = getenv("COAX_CHECK_CACHE");
  if (dir && *dir) return dir;
  const char *home = getenv("HOME");
  return std::string(home && *home ? home : "/tmp") + "/.cache/coax-check";
}

// The SHA-256 of this compiler's executable, once a process; "" in the
// build-only stage0, which can't hash itself and so keeps no verdicts.
static std::string compiler_hash() {
  static std::string hash;
  static bool known = false;
#ifndef COAX_BOOTSTRAP_IMPORTER
  if (!known) hash = profile_file_hash(executable_path());
#endif
  known = true;
  return hash;
}

// Writes record to dir/key through a temporary file, so a reader sees all of
// it or nothing.
static void cache_record(const std::string &dir, const std::string &key, const std::string &record) {
  std::error_code ec;
  if (key.empty() || !(std::filesystem::create_directories(dir, ec), !ec || std::filesystem::is_directory(dir, ec))) return;
  const std::string temp = dir + "/." + key + "." + std::to_string(getpid());
  if (write_file(temp, record)) std::rename(temp.c_str(), (dir + "/" + key).c_str());
}

// The units unit `at` loads, directly or not, itself first, in a fixed order.
static std::vector<int32_t> library_closure(const Source *src, int32_t at) {
  std::vector<int32_t> out{at};
  std::set<int32_t> seen{at};
  for (size_t i = 0; i < out.size(); i++)
    for (int32_t next : src->units[(size_t)out[i]]->loads)
      if (seen.insert(next).second) out.push_back(next);
  return out;
}

// The cache key of unit `at`'s standalone check, or "" when there is none.
static std::string library_key(const Source *src, int32_t at, const EmitOptions &opts, const std::string &compiler, bool run) {
  if (compiler.empty()) return "";
  namespace fs = std::filesystem;
  const fs::path base = fs::path(src->units[(size_t)at]->canonical).parent_path();
  std::string text = "coax.library-check.v1\n" + compiler + "\n" + (run ? "run\n" : "check\n");
  text += "run_steps=" + std::to_string(opts.run_steps) + " development=" + std::to_string(opts.development) +
          " wrapping=" + std::to_string(opts.wrapping) + " profile=" + std::to_string(opts.profile_identity) +
          " metal=" + std::to_string(opts.metal) + " cuda=" + std::to_string(opts.cuda) + " metal_compute=" +
          std::to_string(opts.metal_compute) + " quantity_device=" + std::to_string(opts.quantity_device) +
          (g_check_slots ? " check_slots=" + std::to_string(g_check_slots) : std::string()) + "\n";
  for (int32_t u : library_closure(src, at)) {
    const SourceUnit &unit = *src->units[(size_t)u];
    const std::string sha = debug_source_hash(unit.text);
    if (sha.empty()) return "";
    text += fs::path(unit.canonical).lexically_relative(base).string() + "\t" + sha + "\n";
  }
  return debug_source_hash(text);
}

// Checks unit `at` on its own in a child process: true when it checks (and,
// with run, when its checks pass).
static bool run_compilation_checks(File *file) {
  // Constant evaluation already finished under its smaller limits. Checks
  // have the same published storage budget here as in `coax test`; keep the
  // original limits for every later compiler phase, including failed checks.
  auto &memory = *file->ct_memory;
  struct RestoreLimits {
    CtMemory &memory;
    uint64_t slots, containers;
    ~RestoreLimits() { memory.slot_limit = slots; memory.container_limit = containers; }
  } restore{memory, memory.slot_limit, memory.container_limit};
  memory.slot_limit = g_check_slots ? g_check_slots : CB_test_slot_limit();
  memory.container_limit = CB_test_container_limit();
  g_timings.check_phase = "authority";
  // Each check's time in the last parallel run, beside the check cache
  // ("seconds<TAB>name" lines), so the next run starts the slowest first.
  std::map<std::string, double> times;
  const std::string sha = debug_source_hash("coax.check-times.v1\n" + file->src->units[0]->canonical);
  const std::string dir = library_cache_dir(), path = sha.empty() ? "" : dir + "/" + sha;
  std::string text;
  if (!path.empty() && read_record(path, &text))
    for (size_t at = 0; at < text.size();) {
      size_t tab = text.find('\t', at), end = text.find('\n', at);
      if (end == std::string::npos) break;
      if (tab != std::string::npos && tab < end) times[text.substr(tab + 1, end - tab - 1)] = strtod(text.c_str() + at, nullptr);
      at = end + 1;
    }
  std::vector<PassedCheck> passed;
  const bool parallel = parallel_checks(file, TEST_STEPS, &passed, &times);
  if (!path.empty() && !times.empty()) {
    std::string record;
    char seconds[64];
    for (const auto &[name, value] : times)
      if (name.find('\n') == std::string::npos) { snprintf(seconds, sizeof seconds, "%.3f\t", value); record += seconds + name + "\n"; }
    cache_record(dir, sha, record);
  }
  if (parallel) return run_all_checks(file, TEST_STEPS, false, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, false, &passed);
  return run_all_checks(file, TEST_STEPS, false);
}

static bool library_check_standalone(const Source *src, int32_t at, const EmitOptions &opts, bool run) {
  fflush(stdout);
  fflush(stderr);
  pid_t pid = fork();
  if (pid < 0) return false;
  if (pid == 0) {
    int null = open("/dev/null", O_WRONLY);
    if (null >= 0) { dup2(null, 1); dup2(null, 2); }
    g_timings.path.clear();
    g_timings.table = false;
    g_timings.source_map.clear();
    g_semantic.path.clear();
    g_error_count = 0;
    g_quiet_errors = true;  // its diagnostics would go nowhere; printing a long line's caret is not free
    Source s;
    Arena arena;
    File file;
    bool ok = front_end(src->units[(size_t)at]->canonical.c_str(), &s, &arena, &file, opts, 1, false, false, nullptr);
    if (ok && run) ok = run_compilation_checks(&file);
    _exit(ok && g_error_count == 0 ? 0 : 1);
  }
  int status = 0;
  while (waitpid(pid, &status, 0) < 0) if (errno != EINTR) return false;
  return WIFEXITED(status) && WEXITSTATUS(status) == 0;
}

// #authority check verdicts (ax-authority-check-cache). A compile of a
// program with an #authority unit runs every check it keeps; Everglade's take
// minutes. When they all pass, the compile records that in the check cache
// under the SHA-256 of the compiler binary, the options that change checking
// (the run-step budget used, including the entry file's #run_steps, the
// profile, arithmetic, build profile, library, Metal or CUDA), each unit's
// canonical path and SHA-256, the SHA-256 of the whole input after
// #import_proto, and which units' checks library_units left out. A later
// compile with the same key skips run_compilation_checks. A failure is never
// recorded, so a failing program runs its checks, and reports the same
// diagnostics, every time; a passing one prints nothing either way. coax test,
// mutate and an --evidence record always run the checks.
static std::string authority_key(const Source *src, const File *file, const EmitOptions &opts, const std::vector<bool> &dropped) {
  const std::string compiler = compiler_hash();
  if (compiler.empty()) return "";
  std::string text = "coax.authority-check.v1\n" + compiler + "\n";
  text += "run_steps=" + std::to_string(file->run_steps) + " option_steps=" + std::to_string(opts.run_steps) +
          " given=" + std::to_string(opts.run_steps_given) + " development=" + std::to_string(opts.development) +
          " wrapping=" + std::to_string(opts.wrapping) + " profile=" + std::to_string(opts.profile_identity) +
          " library=" + std::to_string(file->library) + " metal=" + std::to_string(opts.metal) + " cuda=" +
          std::to_string(opts.cuda) + " metal_compute=" + std::to_string(opts.metal_compute) +
          " quantity_device=" + std::to_string(opts.quantity_device) + " check_slots=" +
          std::to_string(g_check_slots ? g_check_slots : CB_test_slot_limit()) + "\n";
  for (size_t u = 0; u < src->units.size(); u++) {
    const SourceUnit &unit = *src->units[u];
    const std::string sha = debug_source_hash(unit.text);
    if (sha.empty()) return "";
    text += unit.canonical + "\t" + sha + (u < dropped.size() && dropped[u] ? "\tlibrary" : "") + "\n";
  }
  const std::string all = debug_source_hash(src->text);
  if (all.empty()) return "";
  return debug_source_hash(text + "input\t" + all + "\n");
}

static bool authority_checks(const Source *src, File *file, const EmitOptions &opts, const std::vector<bool> &dropped, bool mutation_lowering) {
  static const char PASSED[] = "authority checks pass\n";
  const std::string key = mutation_lowering || !g_evidence.path.empty() ? "" : authority_key(src, file, opts, dropped);
  const std::string dir = library_cache_dir();
  std::string record;
  if (!key.empty() && read_record(dir + "/" + key, &record) && record == PASSED) {
    g_authority_cached++;
    return true;
  }
  const int errors = g_error_count;
  const bool ok = run_compilation_checks(file);
  g_authority_checked++;
  if (ok && g_error_count == errors) cache_record(dir, key, PASSED);
  return ok;
}

// Which loaded units' check bodies this compile leaves out (library_checks).
static std::vector<bool> library_units(const Source *src, const std::vector<Token> &tokens, const EmitOptions &opts) {
  const size_t n = src->units.size();
  std::vector<bool> out(n, false);
  if (!g_library_checks || n < 2) return out;
  bool run = false;
  for (const auto &unit : src->units) run = run || unit->authority;
  std::vector<bool> proto(n, false);
  for (const Token &t : tokens)
    if (t.kind == Tok::directive && str_eq(t.text, "import_proto"))
      if (const SourceUnit *u = source_unit_at(src, t.loc))
        for (size_t i = 0; i < n; i++) if (src->units[i].get() == u) proto[i] = true;
  const std::string compiler = compiler_hash();
  const std::string dir = library_cache_dir();
  // Loaded units before their loaders, so a unit's own check reuses its libraries' verdicts.
  std::vector<int32_t> order;
  std::vector<uint8_t> state(n, 0);
  std::function<void(int32_t)> post = [&](int32_t u) {
    if (state[(size_t)u]) return;
    state[(size_t)u] = 1;
    for (int32_t next : src->units[(size_t)u]->loads) post(next);
    order.push_back(u);
  };
  post(0);
  for (int32_t u : order) {
    bool authority = false, imports = false;
    for (int32_t c : library_closure(src, u)) { authority = authority || src->units[(size_t)c]->authority; imports = imports || proto[(size_t)c]; }
    if (!SU_library_checks(u == 0, authority, imports, true)) continue;  // no check can change the answer
    const std::string &canonical = src->units[(size_t)u]->canonical;
    auto known = g_library_verdicts.find(canonical + (run ? "\nrun" : ""));
    if (known != g_library_verdicts.end()) { out[(size_t)u] = SU_library_checks(false, authority, imports, known->second); continue; }
    const auto began = std::chrono::steady_clock::now();
    const size_t cached_before = g_library_cached;
    const std::string key = library_key(src, u, opts, compiler, run);
    std::string record;
    bool verdict;
    if (!key.empty() && read_record(dir + "/" + key, &record) && (record == "checks\n" || record == "does not check alone\n")) {
      verdict = record == "checks\n";
      g_library_cached++;
    } else {
      verdict = library_check_standalone(src, u, opts, run);
      g_library_checked++;
      cache_record(dir, key, verdict ? "checks\n" : "does not check alone\n");
    }
    if (g_timings.on()) g_timings.libraries.push_back({canonical, seconds_since(began), g_library_cached != cached_before, verdict});
    g_library_verdicts[canonical + (run ? "\nrun" : "")] = verdict;
    out[(size_t)u] = SU_library_checks(false, authority, imports, verdict);
  }
  return out;
}

// Removes the tokens of each file-scope #test and #forall (SU_check_spans)
// in the units library_units chose.
static void drop_library_checks(const Source *src, const std::vector<bool> &units, std::vector<Token> *tokens) {
  bool any = false;
  for (bool u : units) any = any || u;
  if (!any) return;
  std::vector<coax_t_SU_Token> records;
  records.reserve(tokens->size());
  for (const Token &t : *tokens) {
    const Str text = t.kind == Tok::string_lit ? t.string_value : t.text;
    records.push_back({(uint32_t)t.kind, {text.data, text.count}, t.loc.offset, t.loc.offset + t.text.count});
  }
  std::vector<coax_t_SU_CheckSpan> spans(tokens->size() / 3 + 1);
  const int64_t n = SU_check_spans({records.data(), (int64_t)records.size()}, {spans.data(), (int64_t)spans.size()});
  if (n < 0) return;  // cannot happen: a span holds at least three tokens
  std::vector<bool> drop(tokens->size(), false);
  for (int64_t i = 0; i < n; i++) {
    const Token &first = (*tokens)[(size_t)spans[(size_t)i].first], &last = (*tokens)[(size_t)spans[(size_t)i].last];
    for (size_t u = 1; u < src->units.size(); u++) {
      const SourceUnit &unit = *src->units[u];
      if (!units[u] || first.loc.offset < unit.begin || first.loc.offset >= unit.end) continue;
      if (last.loc.offset < unit.end)
        for (int64_t k = spans[(size_t)i].first; k <= spans[(size_t)i].last; k++) drop[(size_t)k] = true;
    }
  }
  std::vector<Token> kept;
  kept.reserve(tokens->size());
  for (size_t i = 0; i < tokens->size(); i++) if (!drop[i]) kept.push_back((*tokens)[i]);
  *tokens = std::move(kept);
}

// lex, parse, check and evaluate; then produce the requested text.
// library: no main is required (lib, header, and check of a file that exports).
// header_path: also write the C header (`coax c FILE --library --header out.h`).
// An #authority file's checks run on every compile: a failing check fails
// the build. run_checks is false for coax test and mutate, which run them.
// parsed: coax query's snapshot of the parse; it also stops after checking,
// so constants and defaults keep the expressions their names were written in.
struct QueryParsed;
static void query_snapshot(QueryParsed *parsed, const File *file);
// Published declaration/query inventories have no sum/case records. Refuse the
// complete requested inventory rather than silently omit new checked facts.
// This is a transport-profile limit, not type/handling admission in the host.
struct LegacySumInventory {
  bool found = false;
  Loc witness{};
  std::set<Type> types;
  void mark(Loc loc) { if (!found) witness = loc; found = true; }
  void type(Type t, Loc loc) {
    if (!t || found || !types.insert(t).second) return;
    if (is_sum(t)) { mark(loc); return; }
    if (t->kind == TypeKind::array || t->kind == TypeKind::view) type(t->elem, loc);
    if (t->kind == TypeKind::struct_ && t->struct_decl)
      for (const Field &f : t->struct_decl->fields) type(f.type, f.loc);
  }
  void syntax(const TypeExpr *t) {
    if (!t || found) return;
    if (t->kind == TypeExpr::optional ||
        (t->kind == TypeExpr::named && str_eq(t->name, "Result"))) { mark(t->loc); return; }
    syntax(t->elem); expr(t->length);
    for (const TypeArgument &arg : t->args) { syntax(arg.type); expr(arg.length); }
  }
  void expr(const Expr *e) {
    if (!e || found) return;
    if (e->kind == ExprKind::sum || e->kind == ExprKind::null_ ||
        e->kind == ExprKind::some || e->kind == ExprKind::try_ ||
        e->kind == ExprKind::or_else) { mark(e->loc); return; }
    type(e->type, e->loc); syntax(e->type_expr);
    expr(e->lhs); expr(e->rhs); expr(e->third);
    for (const Expr *a : e->args) expr(a);
    for (const FieldInit &f : e->fields) expr(f.value);
  }
  void stmt(const Stmt *s) {
    if (!s || found) return;
    if (s->kind == StmtKind::switch_) { mark(s->loc); return; }
    type(s->type, s->loc); syntax(s->type_expr);
    expr(s->expr); expr(s->expr2); expr(s->target);
    for (const Stmt *c : s->body) stmt(c);
    stmt(s->then_); stmt(s->else_);
  }
  void proc(const ProcDecl *p) {
    type(p->ret, p->loc); syntax(p->ret_expr);
    for (const Param &a : p->params) { type(a.type, a.loc); syntax(a.type_expr); }
    stmt(p->body);
  }
  bool file(const File *f) {
    for (const UnionDecl *d : f->unions) mark(d->loc);
    for (const StructDecl *d : f->structs)
      for (const Field &a : d->fields) { type(a.type, a.loc); syntax(a.type_expr); expr(a.init); }
    for (const StructDecl *d : f->generic_structs)
      for (const Field &a : d->fields) { syntax(a.type_expr); expr(a.init); }
    for (const ConstDecl *d : f->constants) expr(d->expr);
    for (const ProcDecl *p : f->procs) proc(p);
    for (const ProcDecl *p : f->generics) proc(p);
    for (const CheckDecl *c : f->checks) if (c->proc) proc(c->proc);
    for (const Expr *a : f->assertions) expr(a);
    return found;
  }
};
static bool legacy_sum_procedure(const ProcDecl *proc) {
  LegacySumInventory inventory;
  inventory.proc(proc);
  return inventory.found;
}
static bool front_end(const char *path, Source *src, Arena *arena, File *file, const EmitOptions &opts, int library,
                      bool run_checks = true, bool mutation_lowering = false, QueryParsed *parsed = nullptr);
static bool front_end(const char *path, Source *src, Arena *arena, File *file, const EmitOptions &opts, int library,
                      bool run_checks, bool mutation_lowering, QueryParsed *parsed) {
  {TimedPhase phase{"load"};if (!load_sources(arena, src, path)) return false;}
  for(const auto &unit:src->units)if(!profile_destination_disjoint(unit->canonical))return false;
  std::vector<Token> tokens;
  bool ok;{TimedPhase phase{"lex"};ok=lex(arena, src, &tokens);}
  if (ok) {
    size_t before = src->text.size();
    {TimedPhase phase{"imports"};ok=import_protos(src, opts, tokens);}
    if (ok && src->text.size() != before) {
      tokens.clear();
      TimedPhase phase{"lex"};ok=lex(arena, src, &tokens);
    }
  }
  std::vector<bool> dropped;
  if (ok && g_library_checks) {TimedPhase phase{"libraries"};dropped=library_units(src, tokens, opts);drop_library_checks(src, dropped, &tokens);}
  if(!g_semantic.path.empty()) {
    for(const auto &unit:src->units)g_semantic.sources.emplace_back(unit->canonical,debug_source_hash(unit->text));
    // No report is written when the complete input graph could not be read
    // and checked for aliases (including failures partway through #load).
    g_semantic.inputs_admitted=ok;
  }
  if (ok) {TimedPhase phase{"parse"};ok=parse(arena, src, tokens, file, opts.development);}
  if (ok && parsed) query_snapshot(parsed, file);
  if (ok) {
    bool exports = false;
    for (const ProcDecl *p : file->procs) exports = exports || p->export_c;
    file->library = library == 1 || (library == 2 && exports);
    file->metal = opts.metal;
    file->quantity_device=opts.quantity_device;
    file->execution_profile=std::to_string(opts.profile_identity);
    {TimedPhase phase{"check"};ok=check(arena,file,mutation_lowering);}
    // --run-steps overrides the entry file's #run_steps; either replaces the default.
    if(opts.run_steps_given || !file->run_steps) file->run_steps=opts.run_steps;
    if(ok && !parsed) {TimedPhase phase{"evaluate"};g_timings.run_steps=file->run_steps;ok=evaluate(file,file->run_steps,&g_timings.evaluated_steps,&g_timings.traversal_work);g_timings.peak_slots=file->ct_memory->peak_slots;g_timings.peak_containers=file->ct_memory->peak_containers;g_timings.peak_string_bytes=file->ct_memory->string_peak_bytes;}
  }
  if (ok && !parsed) {TimedPhase phase{"authority"};if(run_checks && file->authority)ok=authority_checks(src,file,opts,dropped,mutation_lowering);}
  if (!ok && !g_json_errors) fprintf(stderr, "%d error%s\n", g_error_count, g_error_count == 1 ? "" : "s");
  return ok;
}

static bool compile(const char *path, const EmitOptions &opts, Output what, std::string *text, int library = 0,
                    const std::string *header_path = nullptr) {
  Source src;
  Arena arena;
  File file;
  g_library_checks = what == Output::c || what == Output::header || what == Output::none;
  if (!front_end(path, &src, &arena, &file, opts, library)) return false;
  if (what == Output::declarations || what == Output::modules) {
    LegacySumInventory inventory;
    if (inventory.file(&file)) {
      error_at(&src, inventory.witness, "E0911", "the requested legacy inventory has no tagged-value or switch records");
      return false;
    }
  }
  if (opts.cuda || opts.metal_compute) {file.cuda=opts.cuda;if(!compute_admit(&file,opts.metal_compute)) {if (!g_json_errors) fprintf(stderr, "%d error%s\n", g_error_count, g_error_count == 1 ? "" : "s");return false;}}
  TimedPhase phase{"emit"};
  if(!g_timings.source_map.empty() && !write_file(g_timings.source_map,emit_debug_map(&file))) {fprintf(stderr,"coax: cannot write source map '%s'\n",g_timings.source_map.c_str());return false;}
  for(const ProcDecl *p:file.procs)if(p->origin)g_timings.instances++;
  if (what == Output::c) {*text=emit_c(&file,opts);g_timings.generated_bytes=text->size();g_timings.instance_bytes=g_instance_c_bytes;}
  else if (what == Output::header) *text = emit_header(&file);
  else if (what == Output::layout) *text = layout_report(&file);
  else if (what == Output::declarations) *text = declarations_report(&src, &file);
  else if (what == Output::modules) *text = modules_report(&src, &file, opts);
  else if (what == Output::lowering) *text = lowering_report(&src, &file, opts);
  else if (what == Output::effects) {if(!effects_report(&src,&file,opts,g_lower_proc,text))return false;}
  else if (what == Output::deps) {
    *text="{\"schema_version\":1,\"run_steps\":" + std::to_string(file.run_steps) + ",\"aggregate_slot_limit\":" + std::to_string(CtMemory::max_slots()) + ",\"aggregate_container_limit\":" + std::to_string(CtMemory::max_containers()) + ",\"aggregate_traversal_limit\":" + std::to_string(CB_traversal_limit()) + ",\"string_live_byte_limit\":" + std::to_string(CB_string_live_limit()) + ",\"string_materialization_work_limit\":" + std::to_string(CB_string_work_limit()) + ",\"sources\":[";
    bool first=true;
    for (const auto &unit:src.units) {*text+=(first?"":",")+json_string(unit->canonical);first=false;}
    *text+="],\"descriptors\":[";first=true;
    for (const auto &name:src.descriptor_inputs) {*text+=(first?"":",")+json_string(name);first=false;}
    *text+="]}\n";
  }
  std::string provenance;
  for (const std::string &line : src.import_provenance) provenance += line + "\n";
  if (what == Output::c || what == Output::header) *text = provenance + *text;
  if (what==Output::c)g_timings.generated_bytes=text->size();
  if(!g_semantic.path.empty() && what!=Output::none)g_semantic.emitted=debug_source_hash(*text);
  if (header_path && !write_file(*header_path, provenance + emit_header(&file))) {
    fprintf(stderr, "coax: cannot write '%s'\n", header_path->c_str());
    return false;
  }
  return true;
}

static std::string strip_ext(const std::string &p) {
  size_t slash = p.find_last_of('/');
  size_t dot = p.find_last_of('.');
  if (dot != std::string::npos && (slash == std::string::npos || dot > slash)) return p.substr(0, dot);
  return p;
}

static int build(const char *path, const std::string &exe, const EmitOptions &opts, bool keep_c) {
  for(const auto &name:{exe,exe+".c",exe+".o"})if(!profile_destination_disjoint(name))return 1;
  std::string c;
  if (!compile(path, opts, Output::c, &c, opts.library ? 1 : 0)) return 1;
  std::string c_path = exe + ".c";
  if (!write_file(c_path, c)) {
    fprintf(stderr, "coax: cannot write '%s'\n", c_path.c_str());
    return 1;
  }
  const char *cc = getenv("COAX_CC");
  std::vector<std::string> args = {cc && *cc ? cc : "cc", "-std=c11", "-w", "-O"+std::to_string(opts.optimization)};
  if (opts.debug_info) args.push_back("-g");
  args.push_back("-ffp-contract=off");
  std::string object=opts.library?exe:exe+".o";
  args.insert(args.end(),{"-c","-o",object,c_path});
  int rc;
  {TimedPhase phase{"native_compile"};rc=run_process(args,&g_semantic.native_invoked);}
  if(rc==0 && !opts.library) {
    std::vector<std::string> link={cc&&*cc?cc:"cc",object,"-o",exe,"-lm"};
    if(opts.debug_info) link.push_back("-g");
    TimedPhase phase{"link"};rc=run_process(link);
  }
  // Darwin's debug map references the object. Keep it for source debugging.
  if(!opts.library && !opts.debug_info)unlink(object.c_str());
  if (!keep_c) unlink(c_path.c_str());
  if (rc != 0) {
    fprintf(stderr, "coax: the C compiler failed on generated code (exit %d); this is a coax bug\n", rc);
    return 1;
  }
  if(!g_semantic.path.empty()) {
    g_semantic.artifact=profile_file_hash(exe);
    if(g_semantic.artifact.empty()){fprintf(stderr,"coax: cannot hash native artifact '%s'\n",exe.c_str());return 1;}
  }
  return 0;
}

static int usage() {
  fprintf(stderr,
          "usage: coax run     file.coax [--release]\n"
          "       coax build   file.coax [-o out] [--release] [--keep-c]\n"
          "       coax lib     file.coax [-o out.o] [--release] [--keep-c]\n"
          "       coax header  file.coax [-o out.h]\n"
          "       coax c       file.coax [--release] [--library [--header out.h]]\n"
          "       coax checks-c file.coax [--verification-abi 1|2] [--only SUBSTR] [-o checks.c]  verification-only checked C\n"
          "       coax metal   file.coax\n"
          "       coax cuda    file.coax [--release]\n"
          "       coax metal-compute file.coax [--release]\n"
          "       coax spirv   file.coax [--kernel NAME | --fill A[,B,C,D] | --stroke NAME | --vertex V --fragment F [--tree]] [-o out.spv]\n"
          "       coax wgsl    file.coax (--functions A,B | --vertices A,B --fragments C,D --bindings F [--strip P]) [--tree] [-o out.wgsl]\n"
          "       coax check   file.coax [--slots N]  (--slots also on run, build, lib, header, c: a smaller slot limit for #authority checks)\n"
          "       coax proto   profile.pb --prefix Name [-o output.coax]\n"
          "       coax layout  file.coax\n"
          "       coax deps    file.coax                           JSON source/descriptor inputs\n"
          "       coax deps    file.coax --declarations            JSON logical owners, declaration IDs, references\n"
          "       coax query   file.coax defines NAME | uses NAME | exports [MODULE] [--json] [--fresh]\n"
          "       coax deps    file.coax --lowering [--proc NAME]  JSON ordered lowering: operations, traps, sequencing\n"
          "       coax deps    file.coax --effects [--proc NAME]   JSON transitive may-effects and witness paths\n"
          "       coax deps    file.coax --modules                 JSON module content and dependency revisions\n"
          "       coax test    file.coax [--steps N] [--slots N] [--only SUBSTR] [--list] [--profile steps] [--evidence FILE]\n"
          "       coax mutate  file.coax [--jobs N] [--shard I/N | --merge RECORD...] [--evidence FILE]\n"
          "                    [--root-only] [--mutation-reference] [--mutation-stats] [--mutation-trace]\n"
          "                    --root-only mutates entry-source procedures; all baseline checks still run\n"
          "                    --jobs N evaluates mutants in N processes (default: available cores,\n"
          "                    or $COAX_MUTATE_JOBS); the report is the same for every N\n"
          "                    --shard I/N runs mutants I-1, I-1+N, ... and prints a record on stdout\n"
          "                    --merge RECORD (repeated) combines all N records into the full report\n"
          "       coax explain CODE | --all [--markdown]\n"
          "       coax version\n"
          "       coax source-digest ROOT          (the hash of ROOT/coax's compiler sources)\n"
          "       coax status [--json]\n"
          "options: --arithmetic checked|wrapping selects runtime integer arithmetic\n       --target native-lp64 names the layout target for layout/deps (others: E0612)\n       --opt 0|1|2|3 selects native optimization for run/build/lib\n       --build-profile development|release selects BUILD_DEBUG source/layout\n       --release retains the legacy wrapping/O2/release/no-debug bundle\n       --profile-report FILE records the resolved profile and source/artifact hashes\n       --run-steps N sets the bounded aggregate compile-time work limit\n       --json prints diagnostics as JSON lines\n       --diagnostics json prints coax.diagnostic.v1 lines: spans, notes and fixes\n       --debug-info maps native code to original Coax source\n       --no-debug-info omits native source debugging independently\n       --source-map FILE writes debugger names/locations (implies --debug-info)\n       --timings FILE writes phase wall times and generated byte count (JSON; --timings-json FILE is the same)\n       --timings-table prints phase, check and library times on stderr\n");
  return 2;
}

static int explain(int argc, char **argv) {
  bool all = false, markdown = false;
  const char *code = nullptr;
  for (int i = 2; i < argc; i++) {
    std::string a = argv[i];
    if (a == "--all") all = true;
    else if (a == "--markdown") markdown = true;
    else code = argv[i];
  }
  if (!all && !code) return usage();
  if (markdown) {
    const char *stage = "";
    for (int32_t i = 0; i < ERROR_COUNT; i++) {
      const ErrorInfo &e = ERRORS[i];
      const char *s = e.code[2] == '1' ? "E01xx: lexer" : e.code[2] == '2' ? "E02xx: parser"
                    : e.code[2] == '3' ? "E03xx: checker" : e.code[2] == '4' ? "E04xx: compile-time evaluation"
                    : e.code[2] == '5' ? "E05xx: proto imports" : e.code[2] == '6' ? "E06xx: source units"
                    : e.code[2] == '7' ? "E07xx: device code" : e.code[2] == '8' ? "E08xx: generic structs"
                    : e.code[2] == '9' ? "E09xx: tagged values and effect declarations" : "Other stages";
      if (strcmp(s, stage) != 0) {
        printf("\n## %s\n\n| Code | Meaning | What to do |\n| --- | --- | --- |\n", s);
        stage = s;
      }
      std::string text = e.explanation;
      for (size_t at = 0; (at = text.find('|', at)) != std::string::npos; at += 2) text.replace(at, 1, "\\|");
      printf("| `%s` | %s | %s |\n", e.code, e.title, text.c_str());
    }
    return 0;
  }
  bool found = false;
  for (int32_t i = 0; i < ERROR_COUNT; i++) {
    const ErrorInfo &e = ERRORS[i];
    if (!all && strcmp(e.code, code) != 0) continue;
    printf("%s: %s\n  %s\n", e.code, e.title, e.explanation);
    found = true;
  }
  if (!found) {
    fprintf(stderr, "coax: no error code '%s'\n", code);
    return 1;
  }
  return 0;
}

// The compiler's own sources in a checkout: every regular file under
// coax/src, coax/bootstrap, coax/lib/compiler and coax/lib/proto, hashed as
// sorted (path relative to coax/, content hash) pairs. coax/build.sh writes
// this digest to coax/bin/source-digest when it installs a compiler, so a
// failing command can tell the compiler is older than its checkout. Empty when
// the tree has no compiler sources or the build has no hash (stage0).
static std::string compiler_source_digest(const std::string &root) {
  namespace fs = std::filesystem;
  std::error_code ec;
  const fs::path coax = fs::path(root) / "coax";
  std::vector<std::string> files;
  for (const char *dir : {"src", "bootstrap", "lib/compiler", "lib/proto"}) {
    const fs::path base = coax / dir;
    if (!fs::is_directory(base, ec)) return "";
    for (fs::recursive_directory_iterator it(base, ec), end; !ec && it != end; it.increment(ec))
      if (it->is_regular_file(ec)) files.push_back(fs::relative(it->path(), coax, ec).generic_string());
    if (ec) return "";
  }
  std::sort(files.begin(), files.end());
  std::string listing = "coax-source-digest-v1";
  listing.push_back('\0');
  for (const std::string &name : files) {
    std::string hash = profile_file_hash((coax / name).string());
    if (hash.empty()) return "";
    listing += name;
    listing.push_back('\0');
    listing += hash;
    listing.push_back('\0');
  }
  return debug_source_hash(listing);
}

static std::string executable_path() {
  char self[4096];
#ifdef __APPLE__
  uint32_t size = sizeof self;
  if (_NSGetExecutablePath(self, &size) != 0) return "";
#else
  ssize_t n = readlink("/proc/self/exe", self, sizeof self - 1);
  if (n <= 0) return "";
  self[n] = 0;
#endif
  std::error_code ec;
  auto path = std::filesystem::weakly_canonical(self, ec);
  return ec ? std::string(self) : path.string();
}

#include "query.cpp"
static void query_snapshot(QueryParsed *parsed, const File *file) { parsed->file(file); }

// coax query FILE defines NAME | uses NAME | exports [MODULE] [--json] [--fresh]
static int query_command(const char *input, const EmitOptions &opts, QueryRequest request) {
  namespace fs = std::filesystem;
  const std::string compiler = profile_file_hash(executable_path());
  if (compiler.empty()) {
    fprintf(stderr, "coax: query needs the final compiler, which can hash itself; build-only stage0 cannot\n");
    return 1;
  }
  std::error_code ec;
  const fs::path entry = fs::canonical(input, ec);
  if (ec) {
    fprintf(stderr, "coax: query: cannot read '%s'\n", input);
    return 1;
  }
  const std::string dir = query_cache_dir();
  // One file per entry unit: a different compiler's index for it is replaced.
  const std::string cache = dir + "/" + debug_source_hash(entry.string()) + ".index";
  QueryIndex index;
  std::string text;
  bool warm = false;
  if (!request.fresh && read_record(cache, &text)) {
    warm = query_parse(text, &index) && query_current(index, compiler, entry.string(), opts);
    if (!warm) index = QueryIndex{};
  }
  if (!warm) {
    Source src;
    Arena arena;
    File file;
    QueryParsed parsed;
    if (!front_end(input, &src, &arena, &file, opts, opts.library ? 1 : 2, false, false, &parsed)) return 1;
    LegacySumInventory inventory;
    if (inventory.file(&file)) {
      error_at(&src, inventory.witness, "E0911", "coax.query.v1 has no tagged-value or switch inventory; no index was published");
      return 1;
    }
    index.compiler = compiler;
    index.entry = entry.string();
    index.profile = opts.profile_identity;
    index.library = opts.library;
    QueryBuilder{&src, &file, &parsed, &index}.build();
    // A stale or unwritable cache only costs time; the answer comes from the fresh index.
    fs::create_directories(dir, ec);
    const std::string temp = cache + "." + std::to_string(getpid()) + ".tmp";
    if (!ec && write_file(temp, query_serialize(index))) {
      if (rename(temp.c_str(), cache.c_str()) != 0) unlink(temp.c_str());
    }
  }
  std::string out;
  if (!query_answer(index, request, &out)) return 1;
  fwrite(out.data(), 1, out.size(), stdout);
  return 0;
}

// After a failed command: when this compiler sits at ROOT/coax/bin/coax with
// the digest coax/build.sh wrote, and ROOT's compiler sources no longer match
// it, say so and name the rebuild. Nothing runs when the command succeeds.
static void report_stale_compiler() {
  namespace fs = std::filesystem;
  const fs::path exe = executable_path();
  if (exe.empty() || exe.parent_path().filename() != "bin" || exe.parent_path().parent_path().filename() != "coax") return;
  std::string recorded;
  if (!read_record((exe.parent_path() / "source-digest").string(), &recorded)) return;
  while (!recorded.empty() && (recorded.back() == '\n' || recorded.back() == '\r')) recorded.pop_back();
  const fs::path root = exe.parent_path().parent_path().parent_path();
  const std::string now = compiler_source_digest(root.string());
  if (recorded.empty() || now.empty() || now == recorded) return;
  if (g_diagnostics_json) {
    diagnostic_json("note", "", "this compiler is out of date: " + root.string() + "/coax has different compiler sources than the ones it was built from, so the errors above may come from the old compiler",
                    "null", "", "[{\"message\":" + json_string("rebuild it with " + root.string() + "/coax/build.sh, then run the command again") + ",\"edits\":[]}]");
    return;
  }
  fprintf(stderr,
          "coax: this compiler is out of date: %s/coax has different compiler sources than the ones it was built from,\n"
          "  so the errors above may come from the old compiler. Rebuild it, then run the command again:\n"
          "  %s/coax/build.sh\n",
          root.string().c_str(), root.string().c_str());
}

static int main_impl(int argc, char **argv) {
  if (argc < 2) return usage();
  std::string cmd = argv[1];
  if (cmd == "version" || cmd == "--version") {
    puts(VERSION);
    return 0;
  }
  if (cmd == "source-digest") {
    if (argc != 3) return usage();
    std::string digest = compiler_source_digest(argv[2]);
    if (digest.empty()) {
      fprintf(stderr, "coax: no compiler sources to hash under '%s/coax'\n", argv[2]);
      return 1;
    }
    puts(digest.c_str());
    return 0;
  }
  if (cmd == "status") {
    if (argc==2) fputs(COAX_RELEASE_STATUS_MARKDOWN,stdout);
    else if(argc==3 && std::string(argv[2])=="--json") fputs(COAX_RELEASE_STATUS_JSON,stdout);
    else return usage();
    return 0;
  }
  if (cmd == "explain") return explain(argc, argv);
  if (cmd != "run" && cmd != "build" && cmd != "c" && cmd != "checks-c" && cmd != "check" && cmd != "lib" && cmd != "header" &&
      cmd != "layout" && cmd != "deps" && cmd != "metal" && cmd != "cuda" && cmd != "metal-compute" && cmd != "spirv" && cmd != "wgsl" && cmd != "test" && cmd != "mutate" && cmd != "proto" && cmd != "query")
    return usage();

  const char *input = nullptr;
  std::string out, header, proto_prefix, kernel, fill, stroke, vertex, fragment, functions, vertices, fragments, bindings, strip;
  bool shader_tree = false;
  EmitOptions opts;
  coax_t_CP_Request request{};request.debug_info=-1;
  std::string arithmetic,optimization,build_profile;
  MutationOptions mutation;
  bool keep_c = false;
  bool declarations = false, modules = false, lowering = false, effects = false;
  bool run_steps_seen = false;
  uint64_t steps = TEST_STEPS;
  const char *only = nullptr;
  uint32_t verification_abi = 1;
  bool verification_abi_seen = false;
  bool list = false, profile_steps = false;
  uint64_t test_slots = 0;
  QueryRequest query;
  for (int i = 2; i < argc; i++) {
    std::string a = argv[i];
    if (a == "--release") request.legacy_release = true;
    else if(a=="--arithmetic") {++request.arithmetic_count;arithmetic=i+1<argc?argv[++i]:"";}
    else if(a=="--opt") {++request.optimization_count;optimization=i+1<argc?argv[++i]:"";}
    else if(a=="--build-profile") {++request.build_profile_count;build_profile=i+1<argc?argv[++i]:"";}
    else if(a=="--debug-info") {++request.debug_count;request.debug_info=1;}
    else if(a=="--no-debug-info") {++request.debug_count;request.debug_info=0;}
    else if(a=="--source-map" && i+1<argc){g_timings.source_map=argv[++i];request.source_map=true;}
    else if(a=="--profile-report") {
      if(!g_semantic.path.empty() || i+1>=argc || !*argv[i+1] || argv[i+1][0]=='-')return usage();
      g_semantic.path=argv[++i];
    }
    else if((a=="--timings" || a=="--timings-json") && i+1<argc)g_timings.path=argv[++i];
    else if(a=="--timings-table")g_timings.table=true;
    else if (a == "--run-steps") {
      if (run_steps_seen || i + 1 >= argc || cmd == "proto") {
        fprintf(stderr, "coax: --run-steps requires one positive decimal value at most 250000000\n");
        return 1;
      }
      const char *value = argv[++i];
      opts.run_steps = CB_parse_steps({value, (int64_t)strlen(value)});
      if (!opts.run_steps) {
        fprintf(stderr, "coax: --run-steps requires one positive decimal value at most 250000000\n");
        return 1;
      }
      run_steps_seen = true;
      opts.run_steps_given = true;
      g_timings.run_steps = opts.run_steps;
    }
    else if (a == "--only" && i + 1 < argc && (cmd == "test" || cmd == "checks-c") && !only) only = argv[++i];
    else if (a == "--verification-abi" && cmd == "checks-c" && i + 1 < argc && !verification_abi_seen) {
      const char *value = argv[++i];
      uint64_t parsed = CB_parse_steps({value, (int64_t)strlen(value)});
      if (!parsed || parsed > UINT32_MAX) return usage();
      verification_abi = (uint32_t)parsed;
      verification_abi_seen = true;
    }
    else if (a == "--list" && cmd == "test") list = true;
    else if (a == "--profile" && cmd == "test" && i + 1 < argc && std::string(argv[i + 1]) == "steps" && !profile_steps) { profile_steps = true; ++i; }
    else if (a == "--slots" && i + 1 < argc && (cmd == "test" || cmd == "check" || cmd == "c" || cmd == "build" || cmd == "run" ||
                                                 cmd == "lib" || cmd == "header")) {
      // A smaller value-slot limit for the checks than the test limit (a shared
      // machine's memory); on a compile, for the checks an #authority file runs.
      const char *value = argv[++i];
      bool digits = *value != '\0';
      for (const char *p = value; *p; p++) digits = digits && *p >= '0' && *p <= '9';
      errno = 0;
      test_slots = strtoull(value, nullptr, 10);
      if (!digits || errno == ERANGE || !test_slots || test_slots > CB_test_slot_limit()) {
        fprintf(stderr, "coax: --slots requires a positive count at most %llu\n", (unsigned long long)CB_test_slot_limit());
        return 1;
      }
      if (cmd != "test") g_check_slots = test_slots;
    }
    else if (a == "--steps" && i + 1 < argc && cmd == "test") {
      const char *value = argv[++i];
      bool digits = *value != '\0';
      for (const char *p = value; *p; p++) digits = digits && *p >= '0' && *p <= '9';
      errno = 0;
      steps = strtoull(value, nullptr, 10);
      if (!digits || errno == ERANGE || !steps) {
        fprintf(stderr, "coax: --steps requires a positive uint64 integer\n");
        return 1;
      }
    }
    else if (a == "--prefix" && i + 1 < argc && cmd == "proto") proto_prefix = argv[++i];
    else if (a == "--kernel" && i + 1 < argc && cmd == "spirv" && kernel.empty() && fill.empty() && stroke.empty()) kernel = argv[++i];
    else if (a == "--fill" && i + 1 < argc && cmd == "spirv" && kernel.empty() && fill.empty() && stroke.empty()) fill = argv[++i];
    else if (a == "--stroke" && i + 1 < argc && cmd == "spirv" && kernel.empty() && fill.empty() && stroke.empty()) stroke = argv[++i];
    else if (a == "--vertex" && i + 1 < argc && cmd == "spirv" && kernel.empty() && fill.empty() && stroke.empty() && vertex.empty()) vertex = argv[++i];
    else if (a == "--tree" && (cmd == "spirv" || cmd == "wgsl")) shader_tree = true;
    else if (a == "--functions" && i + 1 < argc && cmd == "wgsl" && functions.empty()) functions = argv[++i];
    else if (a == "--vertices" && i + 1 < argc && cmd == "wgsl" && vertices.empty()) vertices = argv[++i];
    else if (a == "--fragments" && i + 1 < argc && cmd == "wgsl" && fragments.empty()) fragments = argv[++i];
    else if (a == "--bindings" && i + 1 < argc && cmd == "wgsl" && bindings.empty()) bindings = argv[++i];
    else if (a == "--strip" && i + 1 < argc && cmd == "wgsl" && strip.empty()) strip = argv[++i];
    else if (a == "--fragment" && i + 1 < argc && cmd == "spirv" && kernel.empty() && fill.empty() && stroke.empty() && fragment.empty()) fragment = argv[++i];
    else if (a == "--root-only" && cmd == "mutate") mutation.root_only = true;
    else if (a == "--mutation-reference" && cmd == "mutate") mutation.reference = true;
    else if (a == "--mutation-stats" && cmd == "mutate") mutation.stats = true;
    else if (a == "--mutation-trace" && cmd == "mutate") mutation.trace = true;
    else if (a == "--jobs" && cmd == "mutate") {
      if (mutation.jobs || i + 1 >= argc || !parse_count(argv[i + 1], &mutation.jobs) || !MS_jobs_valid(mutation.jobs)) {
        fprintf(stderr, "coax: --jobs requires one count from 1 to 1024\n");
        return 1;
      }
      ++i;
    }
    else if (a == "--shard" && cmd == "mutate") {
      std::string spec = i + 1 < argc ? argv[++i] : "";
      size_t slash = spec.find('/');
      if (mutation.shards || slash == std::string::npos || !parse_count(spec.substr(0, slash).c_str(), &mutation.shard) ||
          !parse_count(spec.substr(slash + 1).c_str(), &mutation.shards) || !MS_shard_valid(mutation.shard, mutation.shards)) {
        fprintf(stderr, "coax: --shard requires I/N with 1 <= I <= N <= 65536\n");
        return 1;
      }
    }
    else if (a == "--merge" && cmd == "mutate" && i + 1 < argc) mutation.merge.push_back(argv[++i]);
    else if (a == "--evidence" && (cmd == "test" || cmd == "mutate") && i + 1 < argc && g_evidence.path.empty() && *argv[i + 1] && argv[i + 1][0] != '-')
      g_evidence.path = argv[++i];
    else if (a == "--keep-c") keep_c = true;
    else if (a == "--declarations" && cmd == "deps" && !declarations && !modules && !lowering && !effects) declarations = true;
    else if (a == "--modules" && cmd == "deps" && !declarations && !modules && !lowering && !effects) modules = true;
    else if (a == "--lowering" && cmd == "deps" && !declarations && !modules && !lowering && !effects) lowering = true;
    else if (a == "--effects" && cmd == "deps" && !declarations && !modules && !lowering && !effects) effects=true;
    else if (a == "--proc" && (lowering || effects) && i + 1 < argc) g_lower_proc = argv[++i];
    else if (a == "--target" && (cmd == "deps" || cmd == "layout") && i + 1 < argc) g_target = argv[++i];
    else if (a == "--json") g_json_errors = true;
    else if (a == "--diagnostics" && i + 1 < argc && (std::string(argv[i + 1]) == "json" || std::string(argv[i + 1]) == "text")) {
      g_diagnostics_json = std::string(argv[++i]) == "json";
      g_json_errors = g_diagnostics_json;   // no text trailers or notes between the JSON lines
    }
    else if (a == "--lib" || a == "--library") opts.library = true;
    else if (a == "--header" && i + 1 < argc) header = argv[++i];
    else if (a == "-o" && i + 1 < argc) out = argv[++i];
    else if (a == "--fresh" && cmd == "query") query.fresh = true;
    else if (!input && a.size() && a[0] != '-') input = argv[i];
    else if (cmd == "query" && query.form.empty() && (a == "defines" || a == "uses" || a == "exports")) query.form = a;
    else if (cmd == "query" && !query.form.empty() && query.name.empty() && a.size() && a[0] != '-') query.name = a;
    else return usage();
  }
  if (cmd == "query" && (query.form.empty() || (query.form != "exports" && query.name.empty()))) return usage();
  if (!input || (!header.empty() && cmd != "c")) return usage();
  if (cmd == "deps" || cmd == "layout") {
    // Layout facts are qualified for one named target; any other refuses
    // instead of reporting the native LP64 values under another name.
    const auto t = CK_target({g_target.data(), (int64_t)g_target.size()});
    if (!t.supported) {
      fprintf(stderr, "coax: error[E0612]: layout target '%s' is unsupported: %.*s\n", g_target.c_str(), (int)t.reason.count, t.reason.data);
      return 1;
    }
  }
  if (mutation.shards && !mutation.merge.empty()) {
    fprintf(stderr, "coax: --shard runs one part; --merge combines finished parts. Use one of them\n");
    return 1;
  }
#ifdef COAX_BOOTSTRAP_IMPORTER
  if(cmd=="checks-c"){fprintf(stderr,"coax: native check emission requires the final compiler, not build-only stage0\n");return 1;}
  if(!g_timings.source_map.empty()){fprintf(stderr,"coax: source maps require the final compiler, not build-only stage0\n");return 1;}
  if(!g_semantic.path.empty()){fprintf(stderr,"coax: profile reports require the final compiler, not build-only stage0\n");return 1;}
  if(modules){fprintf(stderr,"coax: module revisions require the final compiler, not build-only stage0\n");return 1;}
#endif
  request.arithmetic={arithmetic.data(),(int64_t)arithmetic.size()};
  request.optimization={optimization.data(),(int64_t)optimization.size()};
  request.build_profile={build_profile.data(),(int64_t)build_profile.size()};
  // Legacy proto emission has no runtime profile; new profile choices require
  // one of the source commands admitted by the checked Coax resolver.
  if(cmd!="proto" || request.arithmetic_count || request.optimization_count || request.build_profile_count || request.debug_count || request.source_map || !g_semantic.path.empty()) {
    // coax wgsl takes the shader subset's profile, spirv's (lib/compiler/wgsl.coax)
    const std::string profile_cmd=cmd=="wgsl"?"spirv":cmd=="query"?"deps":cmd;
    auto profile=CP_resolve({profile_cmd.data(),(int64_t)profile_cmd.size()},request);
    if(profile.error!=COAX_CP_Error_none) {
      auto message=CP_message(profile.error);fprintf(stderr,"coax: %.*s\n",(int)message.count,message.data);return 1;
    }
    opts.wrapping=profile.arithmetic==1;opts.optimization=(int)profile.optimization;
    opts.development=profile.development;opts.debug_info=profile.debug_info;opts.profile_identity=(int)profile.identity;
    g_semantic.selection=profile;g_semantic.command=cmd;g_semantic.resolved=true;
    g_semantic.backend=profile.native?"native-c11":cmd=="cuda"?"cuda":cmd=="metal" || cmd=="metal-compute"?"metal":cmd=="spirv" || cmd=="wgsl"?"spirv":cmd=="c" || cmd=="checks-c" || cmd=="header"?"c11":"evaluator";
  }
  for(const auto &name:{std::string(input),out,header,g_timings.source_map,g_timings.path})if(!profile_destination_disjoint(name))return 1;
  if (cmd == "proto") {
    if (proto_prefix.empty()) return usage();
    proto::Result result = proto::import_file(input, proto_prefix, VERSION);
    if (!result.error.empty()) {
      Source source;source.path=input;
      error_at(&source, {1,1,0}, "E0502", result.error);
      return 1;
    }
    if (out.empty()) fwrite(result.source.data(), 1, result.source.size(), stdout);
    else if (!write_file(out, result.source)) {
      fprintf(stderr, "coax: cannot write '%s'\n", out.c_str());
      return 1;
    }
    return 0;
  }
  opts.quantity_device=cmd=="metal" || cmd=="cuda" || cmd=="metal-compute" || cmd=="spirv" || cmd=="wgsl";
  if (cmd == "metal") opts.library = opts.metal = true;
  if (cmd == "cuda") opts.library = opts.cuda = true;
  if (cmd == "metal-compute") opts.library = opts.metal_compute = true;
  if (cmd == "spirv") {
    // A shader (--vertex/--fragment): the ordinary rules, then the shader
    // subset's lowering. Otherwise the Metal scalar profile, checked as for
    // coax metal, then lowered.
    bool shader = !vertex.empty() || !fragment.empty();
    opts.library = true;
    opts.metal = !shader;
    Source src;
    Arena arena;
    File file;
    std::string words, report;
    bool lowered = front_end(input, &src, &arena, &file, opts, 1) &&
                   (shader ? emit_shader(&file, vertex, fragment, shader_tree, &words, &report) : emit_spirv(&file, kernel, fill, stroke, &words));
    if (lowered && shader) fputs(report.c_str(), stderr);
    if (!lowered) {
      if (g_error_count && !g_json_errors) fprintf(stderr, "%d error%s\n", g_error_count, g_error_count == 1 ? "" : "s");
      return 1;
    }
    if (out.empty()) fwrite(words.data(), 1, words.size(), stdout);
    else if (!write_file(out, words)) {
      fprintf(stderr, "coax: cannot write '%s'\n", out.c_str());
      return 1;
    }
    return 0;
  }
  if (cmd == "wgsl") {
    // WGSL functions from the shader subset (lib/compiler/wgsl.coax): the ordinary rules, then the writer
    opts.library = true;
    Source src;
    Arena arena;
    File file;
    std::string text;
    if (!front_end(input, &src, &arena, &file, opts, 1) || !(functions.empty() ? emit_wgsl_module(&file, vertices, fragments, bindings, strip, shader_tree, &text)
                                                                      : emit_wgsl(&file, functions, shader_tree, &text))) {
      if (g_error_count && !g_json_errors) fprintf(stderr, "%d error%s\n", g_error_count, g_error_count == 1 ? "" : "s");
      return 1;
    }
    if (out.empty()) fwrite(text.data(), 1, text.size(), stdout);
    else if (!write_file(out, text)) {
      fprintf(stderr, "coax: cannot write '%s'\n", out.c_str());
      return 1;
    }
    return 0;
  }
  if (cmd == "checks-c") {
    opts.library = true;
    Source src;
    Arena arena;
    File file;
    if (!front_end(input, &src, &arena, &file, opts, 1)) return 1;
    // Output paths are effects: never overwrite a consumed source/descriptor.
    if (!out.empty()) {
      auto aliases_output = [&](const std::string &path) {
        std::error_code first_error, second_error, equivalent_error;
        auto output_path = std::filesystem::weakly_canonical(out, first_error);
        auto input_path = std::filesystem::weakly_canonical(path, second_error);
        return (!first_error && !second_error && output_path == input_path) ||
               std::filesystem::equivalent(out, path, equivalent_error);
      };
      for (const auto &unit : src.units) if (aliases_output(unit->canonical)) {
        fprintf(stderr, "coax: check C output aliases source '%s'\n", unit->canonical.c_str());
        return 1;
      }
      for (const auto &path : src.descriptor_inputs) if (aliases_output(path)) {
        fprintf(stderr, "coax: check C output aliases descriptor '%s'\n", path.c_str());
        return 1;
      }
    }
    std::string text;
    {
      TimedPhase phase{"emit"};
      bool emitted = false;
      if (verification_abi == 1) emitted = emit_native_checks_c(&file, opts, only, &text);
      else {
        NativeCheckCPlan plan;
        emitted = plan_native_checks_c(&file, opts, only, verification_abi, &plan) &&
                  prepare_native_check_domains(&file, &plan, TEST_STEPS);
        if (emitted) text = emit_native_checks_c_plan(&file, opts, plan);
      }
      if (!emitted) {
        if (g_error_count && !g_json_errors) fprintf(stderr, "%d error%s\n", g_error_count, g_error_count == 1 ? "" : "s");
        return 1;
      }
    }
    std::string provenance;
    for (const std::string &line : src.import_provenance) provenance += line + "\n";
    text = provenance + text;
    g_timings.generated_bytes = text.size();
    for (const ProcDecl *proc : file.procs) if (proc->origin) ++g_timings.instances;
    g_timings.instance_bytes = g_instance_c_bytes;
    if (!g_semantic.path.empty()) g_semantic.emitted = debug_source_hash(text);
    if (out.empty()) fwrite(text.data(), 1, text.size(), stdout);
    else if (!write_file(out, text)) {
      fprintf(stderr, "coax: cannot write '%s'\n", out.c_str());
      return 1;
    }
    return 0;
  }
  if (cmd == "test" || cmd == "mutate") {
    Source src;
    Arena arena;
    File file;
    if (!profile_destination_disjoint(g_evidence.path)) return 1;
    const uint64_t slots = test_slots ? test_slots : CB_test_slot_limit();
    auto record = [&](int status, bool checked) {
      return write_evidence(cmd, input, &src, opts, checked, status, steps, slots, only, list, mutation);
    };
    if (!front_end(input, &src, &arena, &file, opts, opts.library ? 1 : 2, false, cmd == "mutate")) return record(1, false);
    // Checks get their own, larger storage limits (budget.coax): a test may hold a whole world.
    file.ct_memory->slot_limit = slots;
    file.ct_memory->container_limit = CB_test_container_limit();
    return record(cmd == "test" ? test_command(&file, steps, only, list, profile_steps) : mutate_command(&arena, &file, mutation), true);
  }
  if (cmd == "query") {
    query.json = g_json_errors;
    return query_command(input, opts, query);
  }
  std::string text;
  if (cmd == "check") return compile(input, opts, Output::none, &text, opts.library ? 1 : 2) ? 0 : 1;
  if (cmd == "layout" || cmd == "deps") {
    if (!compile(input, opts, cmd == "deps" ? (declarations ? Output::declarations : modules ? Output::modules : lowering ? Output::lowering : effects ? Output::effects : Output::deps) : Output::layout, &text, 2)) return 1;
    fwrite(text.data(), 1, text.size(), stdout);
    return 0;
  }
  if (cmd == "c" || cmd == "header" || cmd == "metal" || cmd == "cuda" || cmd == "metal-compute") {
    if (!compile(input, opts, cmd == "header" ? Output::header : Output::c, &text,
                 cmd != "c" || opts.library ? 1 : 0, header.empty() ? nullptr : &header))
      return 1;
    if (out.empty()) {
      fwrite(text.data(), 1, text.size(), stdout);
    } else if (!write_file(out, text)) {
      fprintf(stderr, "coax: cannot write '%s'\n", out.c_str());
      return 1;
    }
    return 0;
  }
  if (cmd == "build") return build(input, out.empty() ? strip_ext(input) : out, opts, keep_c);
  if (cmd == "lib") {
    opts.library = true;
    return build(input, out.empty() ? strip_ext(input) + ".o" : out, opts, keep_c);
  }

  // run: build into a fresh temporary directory, run, clean up.
  const char *tmp = getenv("TMPDIR");
  std::string dir = std::string(tmp && *tmp ? tmp : "/tmp");
  if (dir.back() != '/') dir += '/';
  dir += "coax-run-XXXXXX";
  if (!mkdtemp(&dir[0])) {
    fprintf(stderr, "coax: cannot create a temporary directory\n");
    return 1;
  }
  std::string exe = dir + "/program";
  int rc = build(input, exe, opts, false);
  if (rc == 0) {
    fflush(stdout);
    rc = run_process({exe});
  }
  run_process({"rm", "-rf", dir});  // the program, and the .dSYM a -g build leaves on macOS
  return rc;
}

}  // namespace coax

int main(int argc, char **argv) {
  int status = coax::finish_profile(coax::main_impl(argc, argv));
  if (status != 0) coax::report_stale_compiler();
  return status;
}
