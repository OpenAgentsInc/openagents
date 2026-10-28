// coax0 driver, and the unity build: this file includes every other source,
// so the compiler builds with one compiler invocation (performance stack
// item 2).
//
//   coax run     file.coax [--release]               compile and run
//   coax build   file.coax [-o out] [--release] [--keep-c]
//   coax lib     file.coax [-o out.o] [--release] [--keep-c]
//                                                    an object file of the #export_c procedures
//   coax header  file.coax [-o out.h]                the C header for those procedures
//   coax c       file.coax [--release] [--lib]       print the generated C
//   coax check   file.coax                           check without invoking C
//   coax layout  file.coax                           struct and enum layouts
//   coax test    file.coax [--steps N]               run every #test and #forall
//   coax mutate  file.coax                           mutate the code; every mutant must fail a check
//   coax explain CODE | --all [--markdown]           what an error code means
//   coax version
//
// --json prints diagnostics as one JSON object per line on stderr.
// The C compiler is $COAX_CC, or cc.
#include "coax.h"

#include "check.cpp"
#include "errors.cpp"
#include "eval.cpp"
#include "emit_c.cpp"
#include "lex.cpp"
#include "parse.cpp"
#include "verify.cpp"
#include "proto.cpp"

#include <filesystem>
#include <cerrno>
#include <set>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

namespace coax {

static const char *VERSION = "coax0 0.6.0";

int32_t g_error_count = 0;
bool g_json_errors = false;

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

static std::string json_string(const std::string &s) {
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

void error_at(const Source *src, Loc loc, const char *code, const std::string &message, const std::string &fix) {
  g_error_count++;
  std::string reported = message;
  for (const GeneratedRegion &region : src->generated)
    if (loc.offset >= region.start && loc.offset < region.end)
      reported += " (generated proto import " + region.prefix + ")";
  loc = diagnostic_loc(src, loc);
  if (g_json_errors) {
    fprintf(stderr, "{\"file\": %s, \"line\": %d, \"col\": %d, \"code\": \"%s\", \"message\": %s, \"fix\": %s}\n",
            json_string(src->path).c_str(), loc.line, loc.col, code, json_string(reported).c_str(),
            json_string(fix).c_str());
    return;
  }
  fprintf(stderr, "%s:%d:%d: error[%s]: %s\n", src->path.c_str(), loc.line, loc.col, code, reported.c_str());
  show_source(stderr, src, loc);
  if (!fix.empty()) fprintf(stderr, "  fix: %s\n", fix.c_str());
}

void show_source(FILE *out, const Source *src, Loc loc) {
  loc = diagnostic_loc(src, loc);
  const std::string &t = src->original_text.empty() ? src->text : src->original_text;
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

static bool read_file(const char *path, std::string *out) {
  FILE *f = fopen(path, "rb");
  if (!f) return false;
  char buf[65536];
  size_t n;
  while ((n = fread(buf, 1, sizeof buf, f)) > 0) out->append(buf, n);
  bool ok = !ferror(f);
  fclose(f);
  return ok;
}

static bool write_file(const std::string &path, const std::string &data) {
  FILE *f = fopen(path.c_str(), "wb");
  if (!f) return false;
  bool ok = fwrite(data.data(), 1, data.size(), f) == data.size();
  return fclose(f) == 0 && ok;
}

// Runs argv[0] from PATH and returns its exit status (128+signal if killed).
static int run_process(const std::vector<std::string> &args) {
  std::vector<char *> argv;
  for (const std::string &a : args) argv.push_back((char *)a.c_str());
  argv.push_back(nullptr);
  pid_t pid;
  int spawn_error = posix_spawnp(&pid, argv[0], nullptr, nullptr, argv.data(), environ);
  if (spawn_error != 0) {
    fprintf(stderr, "coax: cannot start '%s'\n", argv[0]);
    return 127;
  }
  int status = 0;
  while (waitpid(pid, &status, 0) < 0) {if (errno != EINTR) return 1;}
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  return 1;
}

// Descriptor parsing and code generation run inside the compiler.
// Generated declarations are appended after user text to preserve user locations.
static bool import_protos(Source *src, const EmitOptions &opts, std::vector<Token> &tokens) {
  struct Request { std::string path, prefix; Loc loc; int end; };
  std::vector<Request> requests;
  std::set<std::string> prefixes;
  int depth = 0;
  for (size_t i = 0; i < tokens.size(); i++) {
    const Token &t = tokens[i];
    if (t.kind == Tok::lbrace) depth++;
    if (t.kind == Tok::rbrace) depth--;
    if (t.kind != Tok::directive || !str_eq(t.text, "import_proto")) continue;
    if (depth || i + 4 >= tokens.size() || tokens[i + 1].kind != Tok::string_lit ||
        !str_eq(tokens[i + 2].text, "as") || tokens[i + 3].kind != Tok::ident || tokens[i + 4].kind != Tok::semicolon) {
      error_at(src, t.loc, "E0501", "expected #import_proto \"profile.pb\" as Prefix; at top level");
      return false;
    }
    if (opts.metal) {
      error_at(src, t.loc, "E0504", "proto codecs cannot be compiled as Metal");
      return false;
    }
    std::string path = to_std(tokens[i + 1].string_value), prefix = to_std(tokens[i + 3].text);
    if (path.find('\0') != std::string::npos || !prefixes.insert(prefix).second) {
      error_at(src, t.loc, "E0501", "NUL path or duplicate proto prefix");
      return false;
    }
    requests.push_back({path, prefix, t.loc, tokens[i + 4].loc.offset + 1});
    i += 4;
  }
  if (requests.empty()) return true;
  if (requests.size() > 8) {
    error_at(src, requests[0].loc, "E0502", "at most eight descriptor imports per source");
    return false;
  }
  src->original_text = src->text;
  std::string generated;
  for (const Request &r : requests) {
    std::filesystem::path path(r.path);
    if (path.is_relative()) path = std::filesystem::absolute(std::filesystem::path(src->path).parent_path() / path);
    proto::Result result = proto::import_file(path.lexically_normal().string(), r.prefix, VERSION);
    if (!result.error.empty()) {
      error_at(src, r.loc, "E0502", result.error);
      return false;
    }
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

enum class Output : uint8_t { none, c, header, layout };

// lex, parse, check and evaluate; then produce the requested text.
// library: no main is required (lib, header, and check of a file that exports).
// header_path: also write the C header (`coax c FILE --library --header out.h`).
// An #authority file's checks run on every compile: a failing check fails
// the build. run_checks is false for coax test and mutate, which run them.
static bool front_end(const char *path, Source *src, Arena *arena, File *file, const EmitOptions &opts, int library,
                      bool run_checks = true) {
  src->path = path;
  if (!read_file(path, &src->text)) {
    fprintf(stderr, "coax: cannot read '%s'\n", path);
    return false;
  }
  std::vector<Token> tokens;
  bool ok = lex(arena, src, &tokens);
  if (ok) {
    size_t before = src->text.size();
    ok = import_protos(src, opts, tokens);
    if (ok && src->text.size() != before) {
      tokens.clear();
      ok = lex(arena, src, &tokens);
    }
  }
  if (ok) ok = parse(arena, src, tokens, file);
  if (ok) {
    bool exports = false;
    for (const ProcDecl *p : file->procs) exports = exports || p->export_c;
    file->library = library == 1 || (library == 2 && exports);
    file->metal = opts.metal;
    ok = check(arena, file) && evaluate(file);
  }
  if (ok && run_checks && file->authority) ok = run_all_checks(file, TEST_STEPS, false);
  if (!ok && !g_json_errors) fprintf(stderr, "%d error%s\n", g_error_count, g_error_count == 1 ? "" : "s");
  return ok;
}

static bool compile(const char *path, const EmitOptions &opts, Output what, std::string *text, int library = 0,
                    const std::string *header_path = nullptr) {
  Source src;
  Arena arena;
  File file;
  if (!front_end(path, &src, &arena, &file, opts, library)) return false;
  if (what == Output::c) *text = emit_c(&file, opts);
  else if (what == Output::header) *text = emit_header(&file);
  else if (what == Output::layout) *text = layout_report(&file);
  std::string provenance;
  for (const std::string &line : src.import_provenance) provenance += line + "\n";
  if (what == Output::c || what == Output::header) *text = provenance + *text;
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
  std::string c;
  if (!compile(path, opts, Output::c, &c, opts.library ? 1 : 0)) return 1;
  std::string c_path = exe + ".c";
  if (!write_file(c_path, c)) {
    fprintf(stderr, "coax: cannot write '%s'\n", c_path.c_str());
    return 1;
  }
  const char *cc = getenv("COAX_CC");
  std::vector<std::string> args = {cc && *cc ? cc : "cc", "-std=c11", "-w", opts.release ? "-O2" : "-O0"};
  if (!opts.release) args.push_back("-g");
  if (opts.library) args.push_back("-c");
  args.insert(args.end(), {"-o", exe, c_path});
  if (!opts.library) args.push_back("-lm");
  int rc = run_process(args);
  if (!keep_c) unlink(c_path.c_str());
  if (rc != 0) {
    fprintf(stderr, "coax: the C compiler failed on generated code (exit %d); this is a coax bug\n", rc);
    return 1;
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
          "       coax metal   file.coax\n"
          "       coax check   file.coax\n"
          "       coax proto   profile.pb --prefix Name [-o output.coax]\n"
          "       coax layout  file.coax\n"
          "       coax test    file.coax [--steps N]\n"
          "       coax mutate  file.coax\n"
          "       coax explain CODE | --all [--markdown]\n"
          "       coax version\n"
          "options: --json prints diagnostics as JSON lines\n");
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
                    : e.code[2] == '3' ? "E03xx: checker" : e.code[2] == '4' ? "E04xx: compile-time evaluation" : "E05xx: proto imports";
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

static int main_impl(int argc, char **argv) {
  if (argc < 2) return usage();
  std::string cmd = argv[1];
  if (cmd == "version" || cmd == "--version") {
    puts(VERSION);
    return 0;
  }
  if (cmd == "explain") return explain(argc, argv);
  if (cmd != "run" && cmd != "build" && cmd != "c" && cmd != "check" && cmd != "lib" && cmd != "header" &&
      cmd != "layout" && cmd != "metal" && cmd != "test" && cmd != "mutate" && cmd != "proto")
    return usage();

  const char *input = nullptr;
  std::string out, header, proto_prefix;
  EmitOptions opts;
  bool keep_c = false;
  uint64_t steps = TEST_STEPS;
  for (int i = 2; i < argc; i++) {
    std::string a = argv[i];
    if (a == "--release") opts.release = true;
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
    else if (a == "--keep-c") keep_c = true;
    else if (a == "--json") g_json_errors = true;
    else if (a == "--lib" || a == "--library") opts.library = true;
    else if (a == "--header" && i + 1 < argc) header = argv[++i];
    else if (a == "-o" && i + 1 < argc) out = argv[++i];
    else if (!input && a.size() && a[0] != '-') input = argv[i];
    else return usage();
  }
  if (!input || (!header.empty() && cmd != "c")) return usage();
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
  if (cmd == "metal") opts.library = opts.metal = true;
  if (cmd == "test" || cmd == "mutate") {
    Source src;
    Arena arena;
    File file;
    if (!front_end(input, &src, &arena, &file, opts, 2, false)) return 1;
    return cmd == "test" ? test_command(&file, steps) : mutate_command(&arena, &file);
  }
  std::string text;
  if (cmd == "check") return compile(input, opts, Output::none, &text, opts.library ? 1 : 2) ? 0 : 1;
  if (cmd == "layout") {
    if (!compile(input, opts, Output::layout, &text, 2)) return 1;
    fwrite(text.data(), 1, text.size(), stdout);
    return 0;
  }
  if (cmd == "c" || cmd == "header" || cmd == "metal") {
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

int main(int argc, char **argv) { return coax::main_impl(argc, argv); }
