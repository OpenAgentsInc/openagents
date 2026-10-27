// Parser: tokens to a syntax tree. Recursive descent; stops at the first
// syntax error so every reported error is a real one.
#include "coax.h"

namespace coax {

namespace {

struct Parser {
  Arena *arena;
  const Source *src;
  const std::vector<Token> *toks;
  int32_t pos = 0;
  bool failed = false;

  const Token &peek(int32_t ahead = 0) const {
    int32_t i = pos + ahead;
    if (i >= (int32_t)toks->size()) i = (int32_t)toks->size() - 1;
    return (*toks)[(size_t)i];
  }
  bool at(Tok k) const { return peek().kind == k; }
  bool at_keyword(const char *kw) const { return peek().kind == Tok::keyword && str_eq(peek().text, kw); }
  Token next() { Token t = peek(); if (pos < (int32_t)toks->size() - 1) pos++; return t; }

  void fail(Loc loc, const char *code, const std::string &msg, const std::string &fix = "") {
    if (failed) return;
    failed = true;
    error_at(src, loc, code, msg, fix);
  }

  bool expect(Tok k, const char *what) {
    if (at(k)) { next(); return true; }
    const Token &t = peek();
    std::string got = t.kind == Tok::ident || t.kind == Tok::keyword ? "'" + to_std(t.text) + "'" : tok_name(t.kind);
    // A missing ';' belongs right after the previous token, not at the start
    // of whatever comes next (often the following line).
    Loc where = t.loc;
    if (k == Tok::semicolon && pos > 0) {
      const Token &prev = (*toks)[(size_t)pos - 1];
      where = Loc{prev.loc.line, prev.loc.col + prev.text.count, prev.loc.offset + prev.text.count};
    }
    fail(where, "E0201", std::string("expected ") + tok_name(k) + " " + what + ", found " + got,
         k == Tok::semicolon ? "add ';'" : "");
    return false;
  }

  // type := IDENT  (coax0 has only named built-in types)
  bool parse_type(Str *text, Loc *loc) {
    const Token &t = peek();
    if (t.kind != Tok::ident && t.kind != Tok::keyword) {
      fail(t.loc, "E0202", std::string("expected a type, found ") + tok_name(t.kind));
      return false;
    }
    *text = t.text;
    *loc = t.loc;
    next();
    return true;
  }

  // ---- expressions, lowest to highest precedence ----

  static int precedence(Tok k) {
    switch (k) {
      case Tok::oror: return 1;
      case Tok::andand: return 2;
      case Tok::eqeq: case Tok::noteq: case Tok::lt: case Tok::le: case Tok::gt: case Tok::ge: return 3;
      case Tok::plus: case Tok::minus: return 4;
      case Tok::star: case Tok::slash: case Tok::percent: return 5;
      default: return 0;
    }
  }

  Expr *expr(int32_t min_prec = 1) {
    Expr *lhs = unary();
    while (lhs && !failed) {
      Tok op = peek().kind;
      int p = precedence(op);
      if (p < min_prec || p == 0) break;
      Loc loc = next().loc;
      Expr *rhs = expr(p + 1);
      if (!rhs) return nullptr;
      // Comparisons do not chain: `a < b < c` is an error, not a surprise.
      if (p == 3 && precedence(peek().kind) == 3) {
        fail(peek().loc, "E0203", "comparisons cannot be chained", "split it with '&&'");
        return nullptr;
      }
      Expr *e = make<Expr>(arena);
      e->kind = ExprKind::binary;
      e->loc = loc;
      e->op = op;
      e->lhs = lhs;
      e->rhs = rhs;
      lhs = e;
    }
    return lhs;
  }

  Expr *unary() {
    if (peek().kind == Tok::directive && str_eq(peek().text, "run")) {
      Token t = next();
      Expr *e = make<Expr>(arena);
      e->kind = ExprKind::run;
      e->loc = t.loc;
      if (!(e->lhs = unary())) return nullptr;
      return e;
    }
    if (at(Tok::minus) || at(Tok::bang)) {
      Token t = next();
      Expr *operand = unary();
      if (!operand) return nullptr;
      Expr *e = make<Expr>(arena);
      e->kind = ExprKind::unary;
      e->loc = t.loc;
      e->op = t.kind;
      e->lhs = operand;
      return e;
    }
    return primary();
  }

  Expr *primary() {
    const Token &t = peek();
    Expr *e = make<Expr>(arena);
    e->loc = t.loc;
    switch (t.kind) {
      case Tok::int_lit:
        e->kind = ExprKind::int_lit;
        e->int_value = t.int_value;
        next();
        return e;
      case Tok::string_lit:
        e->kind = ExprKind::string_lit;
        e->text = t.string_value;
        next();
        return e;
      case Tok::lparen: {
        next();
        Expr *inner = expr();
        if (!inner || !expect(Tok::rparen, "to close '('")) return nullptr;
        return inner;
      }
      case Tok::keyword:
        if (str_eq(t.text, "true") || str_eq(t.text, "false")) {
          e->kind = ExprKind::bool_lit;
          e->int_value = str_eq(t.text, "true");
          next();
          return e;
        }
        fail(t.loc, "E0204", "'" + to_std(t.text) + "' is reserved and not supported by coax0 yet");
        return nullptr;
      case Tok::ident: {
        next();
        e->text = t.text;
        if (!at(Tok::lparen)) {
          e->kind = ExprKind::ident;
          return e;
        }
        next();
        e->kind = ExprKind::call;
        std::vector<Expr *> args;
        while (!at(Tok::rparen) && !failed) {
          Expr *a = expr();
          if (!a) return nullptr;
          args.push_back(a);
          if (!at(Tok::comma)) break;
          next();
        }
        if (!expect(Tok::rparen, "after the arguments")) return nullptr;
        e->args = to_slice(arena, args);
        return e;
      }
      default:
        fail(t.loc, "E0205", std::string("expected an expression, found ") + tok_name(t.kind));
        return nullptr;
    }
  }

  // ---- statements ----

  Stmt *block() {
    Stmt *b = make<Stmt>(arena);
    b->kind = StmtKind::block;
    b->loc = peek().loc;
    if (!expect(Tok::lbrace, "to start a block")) return nullptr;
    std::vector<Stmt *> body;
    while (!at(Tok::rbrace) && !at(Tok::eof) && !failed) {
      Stmt *s = stmt();
      if (!s) return nullptr;
      body.push_back(s);
    }
    if (!expect(Tok::rbrace, "to close the block")) return nullptr;
    b->body = to_slice(arena, body);
    return b;
  }

  Stmt *if_stmt() {
    Stmt *s = make<Stmt>(arena);
    s->kind = StmtKind::if_;
    s->loc = next().loc;  // 'if'
    if (!(s->expr = expr())) return nullptr;
    if (!(s->then_ = block())) return nullptr;
    if (at_keyword("else")) {
      next();
      s->else_ = at_keyword("if") ? if_stmt() : block();
      if (!s->else_) return nullptr;
    }
    return s;
  }

  Stmt *stmt() {
    const Token &t = peek();
    if (t.kind == Tok::lbrace) return block();
    if (t.kind == Tok::keyword) {
      if (str_eq(t.text, "if")) return if_stmt();
      if (str_eq(t.text, "while")) {
        Stmt *s = make<Stmt>(arena);
        s->kind = StmtKind::while_;
        s->loc = next().loc;
        if (!(s->expr = expr())) return nullptr;
        if (!(s->then_ = block())) return nullptr;
        return s;
      }
      if (str_eq(t.text, "return")) {
        Stmt *s = make<Stmt>(arena);
        s->kind = StmtKind::ret;
        s->loc = next().loc;
        if (!at(Tok::semicolon) && !(s->expr = expr())) return nullptr;
        if (!expect(Tok::semicolon, "after the return")) return nullptr;
        return s;
      }
    }

    // Declarations: name := expr;  name: type [= expr | = ---];
    if (t.kind == Tok::ident && (peek(1).kind == Tok::colonequal || peek(1).kind == Tok::colon)) {
      Stmt *s = make<Stmt>(arena);
      s->kind = StmtKind::var;
      s->loc = t.loc;
      s->name = t.text;
      next();
      if (next().kind == Tok::colon) {
        if (!parse_type(&s->type_text, &s->type_loc)) return nullptr;
        if (at(Tok::equal)) {
          next();
          // `---`: the lexer gives '-' '-' '-'
          if (at(Tok::minus) && peek(1).kind == Tok::minus && peek(2).kind == Tok::minus) {
            next(); next(); next();
            s->uninit = true;
          } else if (!(s->expr = expr())) {
            return nullptr;
          }
        }
      } else if (!(s->expr = expr())) {
        return nullptr;
      }
      if (!expect(Tok::semicolon, "after the declaration")) return nullptr;
      return s;
    }
    if (t.kind == Tok::ident && peek(1).kind == Tok::coloncolon) {
      fail(peek(1).loc, "E0206", "local constants and procedures are not supported by coax0 yet");
      return nullptr;
    }

    // Assignment or expression statement.
    Expr *e = expr();
    if (!e) return nullptr;
    Tok k = peek().kind;
    if (k == Tok::equal || k == Tok::plus_equal || k == Tok::minus_equal || k == Tok::star_equal ||
        k == Tok::slash_equal || k == Tok::percent_equal) {
      if (e->kind != ExprKind::ident) {
        fail(e->loc, "E0207", "only a variable can be assigned to");
        return nullptr;
      }
      Stmt *s = make<Stmt>(arena);
      s->kind = StmtKind::assign;
      s->loc = next().loc;
      s->op = k;
      s->name = e->text;
      if (!(s->expr = expr())) return nullptr;
      if (!expect(Tok::semicolon, "after the assignment")) return nullptr;
      return s;
    }
    Stmt *s = make<Stmt>(arena);
    s->kind = StmtKind::expr;
    s->loc = e->loc;
    s->expr = e;
    if (!expect(Tok::semicolon, "after the statement")) return nullptr;
    return s;
  }

  // ---- top level ----

  bool starts_procedure() const {
    if (!at(Tok::lparen)) return false;
    int depth = 0;
    for (int i = 0; peek(i).kind != Tok::eof; i++) {
      if (peek(i).kind == Tok::lparen) depth++;
      if (peek(i).kind == Tok::rparen && --depth == 0)
        return peek(i + 1).kind == Tok::lbrace || peek(i + 1).kind == Tok::arrow;
    }
    return false;
  }

  ProcDecl *proc(const Token &name) {
    ProcDecl *p = make<ProcDecl>(arena);
    p->name = name.text;
    p->loc = name.loc;
    if (!expect(Tok::lparen, "to start the parameter list")) return nullptr;
    std::vector<Param> params;
    while (!at(Tok::rparen) && !failed) {
      Param prm;
      const Token &pn = peek();
      if (pn.kind != Tok::ident) {
        fail(pn.loc, "E0208", std::string("expected a parameter name, found ") + tok_name(pn.kind));
        return nullptr;
      }
      prm.name = pn.text;
      prm.loc = pn.loc;
      next();
      if (!expect(Tok::colon, "after the parameter name")) return nullptr;
      if (!parse_type(&prm.type_text, &prm.type_loc)) return nullptr;
      params.push_back(prm);
      if (!at(Tok::comma)) break;
      next();
    }
    if (!expect(Tok::rparen, "to close the parameter list")) return nullptr;
    if (at(Tok::arrow)) {
      next();
      if (!parse_type(&p->ret_text, &p->ret_loc)) return nullptr;
    }
    p->params = to_slice(arena, params);
    if (!(p->body = block())) return nullptr;
    return p;
  }

  bool file(File *out) {
    std::vector<Import> imports;
    std::vector<ProcDecl *> procs;
    std::vector<ConstDecl *> constants;
    std::vector<Expr *> assertions;
    while (!at(Tok::eof) && !failed) {
      const Token &t = peek();
      if (t.kind == Tok::directive && str_eq(t.text, "import")) {
        next();
        const Token &m = peek();
        if (m.kind != Tok::string_lit) {
          fail(m.loc, "E0209", "expected a module name string after #import", "write #import \"Basic\";");
          break;
        }
        next();
        imports.push_back(Import{m.string_value, m.loc});
        if (!expect(Tok::semicolon, "after the import")) break;
        continue;
      }
      if (t.kind == Tok::directive && str_eq(t.text, "assert")) {
        next();
        Expr *condition = expr();
        if (!condition || !expect(Tok::semicolon, "after #assert")) break;
        assertions.push_back(condition);
        continue;
      }
      if (t.kind == Tok::directive) {
        fail(t.loc, "E0210", "#" + to_std(t.text) + " is not supported by coax0 yet");
        break;
      }
      if (t.kind == Tok::ident && peek(1).kind == Tok::coloncolon) {
        next();
        next();
        if (starts_procedure()) {
          ProcDecl *p = proc(t);
          if (!p) break;
          procs.push_back(p);
        } else {
          ConstDecl *c = make<ConstDecl>(arena);
          c->name = t.text;
          c->loc = t.loc;
          if (!(c->expr = expr()) || !expect(Tok::semicolon, "after the constant")) break;
          constants.push_back(c);
        }
        continue;
      }
      if (t.kind == Tok::ident && (peek(1).kind == Tok::colon || peek(1).kind == Tok::colonequal)) {
        fail(t.loc, "E0212", "global variables are not supported by coax0 yet");
        break;
      }
      fail(t.loc, "E0213", "expected '#import' or a declaration 'name :: ...' at the top level");
      break;
    }
    out->src = src;
    out->imports = to_slice(arena, imports);
    out->procs = to_slice(arena, procs);
    out->constants = to_slice(arena, constants);
    out->assertions = to_slice(arena, assertions);
    return !failed;
  }
};

}  // namespace

bool parse(Arena *arena, const Source *src, const std::vector<Token> &tokens, File *out) {
  Parser p{arena, src, &tokens};
  return p.file(out);
}

}  // namespace coax
