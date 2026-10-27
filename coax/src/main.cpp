// coax0 driver, and the unity build: this file includes every other source,
// so the compiler builds with one compiler invocation (performance stack
// item 2).
//
//   coax run   file.coax [--release]                 compile and run
//   coax build file.coax [-o out] [--release] [--keep-c]
//   coax c     file.coax [--release]                 print the generated C
//   coax check file.coax                             check without invoking C
//   coax version
//
// The C compiler is $COAX_CC, or cc.
#include "coax.h"

#include "check.cpp"
#include "eval.cpp"
#include "emit_c.cpp"
#include "lex.cpp"
#include "parse.cpp"

#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

namespace coax {

static const char *VERSION = "coax0 0.2.0";

int32_t g_error_count = 0;

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

void error_at(const Source *src, Loc loc, const char *code, const std::string &message, const std::string &fix) {
  g_error_count++;
  fprintf(stderr, "%s:%d:%d: error[%s]: %s\n", src->path.c_str(), loc.line, loc.col, code, message.c_str());
  // The source line and a caret under the column.
  const std::string &t = src->text;
  size_t start = (size_t)loc.offset;
  if (start > t.size()) start = t.size();
  while (start > 0 && t[start - 1] != '\n') start--;
  size_t end = start;
  while (end < t.size() && t[end] != '\n') end++;
  if (end > start) {
    fprintf(stderr, "  %.*s\n  ", (int)(end - start), t.data() + start);
    for (size_t i = start; i < (size_t)loc.offset && i < end; i++) fputc(t[i] == '\t' ? '\t' : ' ', stderr);
    fputs("^\n", stderr);
  }
  if (!fix.empty()) fprintf(stderr, "  fix: %s\n", fix.c_str());
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
  if (posix_spawnp(&pid, argv[0], nullptr, nullptr, argv.data(), environ) != 0) {
    fprintf(stderr, "coax: cannot start '%s'\n", argv[0]);
    return 127;
  }
  int status = 0;
  while (waitpid(pid, &status, 0) < 0) {
  }
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  return 1;
}

static bool compile_to_c(const char *path, const EmitOptions &opts, std::string *c_out) {
  Source src;
  src.path = path;
  if (!read_file(path, &src.text)) {
    fprintf(stderr, "coax: cannot read '%s'\n", path);
    return false;
  }
  Arena arena;
  std::vector<Token> tokens;
  File file;
  if (!lex(&arena, &src, &tokens) || !parse(&arena, &src, tokens, &file) || !check(&arena, &file) ||
      !evaluate(&file)) {
    fprintf(stderr, "%d error%s\n", g_error_count, g_error_count == 1 ? "" : "s");
    return false;
  }
  if (c_out) *c_out = emit_c(&file, opts);
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
  if (!compile_to_c(path, opts, &c)) return 1;
  std::string c_path = exe + ".c";
  if (!write_file(c_path, c)) {
    fprintf(stderr, "coax: cannot write '%s'\n", c_path.c_str());
    return 1;
  }
  const char *cc = getenv("COAX_CC");
  std::vector<std::string> args = {cc && *cc ? cc : "cc", "-std=c11", "-w", "-o", exe, c_path};
  args.insert(args.begin() + 2, opts.release ? "-O2" : "-O0");
  if (!opts.release) args.insert(args.begin() + 3, "-g");
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
          "usage: coax run   file.coax [--release]\n"
          "       coax build file.coax [-o out] [--release] [--keep-c]\n"
          "       coax c     file.coax [--release]\n"
          "       coax check file.coax\n"
          "       coax version\n");
  return 2;
}

static int main_impl(int argc, char **argv) {
  if (argc < 2) return usage();
  std::string cmd = argv[1];
  if (cmd == "version" || cmd == "--version") {
    puts(VERSION);
    return 0;
  }
  if (cmd != "run" && cmd != "build" && cmd != "c" && cmd != "check") return usage();

  const char *input = nullptr;
  std::string out;
  EmitOptions opts;
  bool keep_c = false;
  for (int i = 2; i < argc; i++) {
    std::string a = argv[i];
    if (a == "--release") opts.release = true;
    else if (a == "--keep-c") keep_c = true;
    else if (a == "-o" && i + 1 < argc) out = argv[++i];
    else if (!input && a.size() && a[0] != '-') input = argv[i];
    else return usage();
  }
  if (!input) return usage();
  if (cmd == "check") return compile_to_c(input, opts, nullptr) ? 0 : 1;

  if (cmd == "c") {
    std::string c;
    if (!compile_to_c(input, opts, &c)) return 1;
    fwrite(c.data(), 1, c.size(), stdout);
    return 0;
  }
  if (cmd == "build") return build(input, out.empty() ? strip_ext(input) : out, opts, keep_c);

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
