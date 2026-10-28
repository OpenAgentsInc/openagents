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
  bool at_keyword(const char *kw, int32_t ahead = 0) const {
    return peek(ahead).kind == Tok::keyword && str_eq(peek(ahead).text, kw);
  }
  Token next() { Token t = peek(); if (pos < (int32_t)toks->size() - 1) pos++; return t; }

  void fail(Loc loc, const char *code, const std::string &msg, const std::string &fix = "") {
    if (failed) return;
    failed = true;
    error_at(src, loc, code, msg, fix);
  }

  std::string describe(const Token &t) const {
    return t.kind == Tok::ident || t.kind == Tok::keyword ? "'" + to_std(t.text) + "'" : tok_name(t.kind);
  }

  bool expect(Tok k, const char *what) {
    if (at(k)) { next(); return true; }
    const Token &t = peek();
    // A missing ';' belongs right after the previous token, not at the start
    // of whatever comes next (often the following line).
    Loc where = t.loc;
    if (k == Tok::semicolon && pos > 0) {
      const Token &prev = (*toks)[(size_t)pos - 1];
      where = Loc{prev.loc.line, prev.loc.col + prev.text.count, prev.loc.offset + prev.text.count};
    }
    fail(where, "E0201", std::string("expected ") + tok_name(k) + " " + what + ", found " + describe(t),
         k == Tok::semicolon ? "add ';'" : "");
    return false;
  }

  bool expect_keyword(const char *kw, const char *what) {
    if (at_keyword(kw)) { next(); return true; }
    fail(peek().loc, "E0201", std::string("expected '") + kw + "' " + what + ", found " + describe(peek()));
    return false;
  }

  bool ident(Str *name, Loc *loc, const char *what) {
    const Token &t = peek();
    if (t.kind != Tok::ident) {
      fail(t.loc, "E0208", std::string("expected ") + what + ", found " + describe(t));
      return false;
    }
    *name = t.text;
    *loc = t.loc;
    next();
    return true;
  }

  // type := NAME | '[' expr ']' type | '[' ']' type | '[' '..' ']' type
  TypeExpr *type() {
    TypeExpr *t = make<TypeExpr>(arena);
    t->loc = peek().loc;
    if (at(Tok::lbracket)) {
      next();
      if (at(Tok::rbracket)) {
        next();
        t->kind = TypeExpr::view;
      } else if (at(Tok::dotdot)) {
        next();
        if (!expect(Tok::rbracket, "after '[..'")) return nullptr;
        t->kind = TypeExpr::dynamic;
      } else {
        t->kind = TypeExpr::array;
        if (!(t->length = expr()) || !expect(Tok::rbracket, "after the array length")) return nullptr;
      }
      if (!(t->elem = type())) return nullptr;
      return t;
    }
    const Token &n = peek();
    if (n.kind != Tok::ident && n.kind != Tok::keyword) {
      fail(n.loc, "E0202", std::string("expected a type, found ") + tok_name(n.kind));
      return nullptr;
    }
    t->name = n.text;
    next();
    return t;
  }

  // ---- expressions, lowest to highest precedence ----
  // Bitwise operators share the levels of + and *, as in Go, so a & b == c
  // means (a & b) == c.

  static int precedence(Tok k) {
    switch (k) {
      case Tok::oror: return 1;
      case Tok::andand: return 2;
      case Tok::eqeq: case Tok::noteq: case Tok::lt: case Tok::le: case Tok::gt: case Tok::ge: return 3;
      case Tok::plus: case Tok::minus: case Tok::pipe: case Tok::caret: return 4;
      case Tok::star: case Tok::slash: case Tok::percent: case Tok::amp: case Tok::shl: case Tok::shr: return 5;
      default: return 0;
    }
  }

  Expr *node(ExprKind kind, Loc loc) {
    Expr *e = make<Expr>(arena);
    e->kind = kind;
    e->loc = loc;
    return e;
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
      Expr *e = node(ExprKind::binary, loc);
      e->op = op;
      e->lhs = lhs;
      e->rhs = rhs;
      lhs = e;
    }
    return lhs;
  }

  Expr *unary() {
    if (peek().kind == Tok::directive && str_eq(peek().text, "run")) {
      Expr *e = node(ExprKind::run, next().loc);
      if (!(e->lhs = unary())) return nullptr;
      return e;
    }
    if (at(Tok::minus) || at(Tok::bang)) {
      Token t = next();
      Expr *operand = unary();
      if (!operand) return nullptr;
      Expr *e = node(ExprKind::unary, t.loc);
      e->op = t.kind;
      e->lhs = operand;
      return e;
    }
    if (at_keyword("cast")) {
      // cast(T) x checks the value fits; cast,trunc(T) x keeps the low bits.
      Expr *e = node(ExprKind::cast, next().loc);
      if (at(Tok::comma)) {
        next();
        if (!(peek().kind == Tok::ident && str_eq(peek().text, "trunc"))) {
          fail(peek().loc, "E0214", "the only cast modifier is 'trunc'", "cast,trunc(u8) x");
          return nullptr;
        }
        next();
        e->trunc = true;
      }
      if (!expect(Tok::lparen, "after 'cast'") || !(e->type_expr = type()) ||
          !expect(Tok::rparen, "after the cast type") || !(e->lhs = unary()))
        return nullptr;
      return e;
    }
    return postfix(primary());
  }

  // x.field  x[i]  x[lo ..< hi]  x[..]  Name.{field = value}
  Expr *postfix(Expr *e) {
    while (e && !failed) {
      if (at(Tok::dot) && peek(1).kind == Tok::ident) {
        Expr *f = node(ExprKind::field, next().loc);
        f->text = next().text;
        f->lhs = e;
        e = f;
      } else if (at(Tok::dot) && peek(1).kind == Tok::lbrace) {
        if (e->kind != ExprKind::ident) {
          fail(peek().loc, "E0215", "a struct literal starts with the struct's name", "Point.{x = 1, y = 2}");
          return nullptr;
        }
        Expr *lit = node(ExprKind::struct_lit, e->loc);
        lit->type_expr = make<TypeExpr>(arena);
        lit->type_expr->name = e->text;
        lit->type_expr->loc = e->loc;
        next();
        next();
        std::vector<FieldInit> fields;
        while (!at(Tok::rbrace) && !failed) {
          FieldInit fi;
          if (!ident(&fi.name, &fi.loc, "a field name") ||
              !expect(Tok::equal, "after the field name; struct literals name every field") ||
              !(fi.value = expr()))
            return nullptr;
          fields.push_back(fi);
          if (!at(Tok::comma)) break;
          next();
        }
        if (!expect(Tok::rbrace, "to close the struct literal")) return nullptr;
        lit->fields = to_slice(arena, fields);
        e = lit;
      } else if (at(Tok::lbracket)) {
        Loc loc = next().loc;
        if (at(Tok::dotdot) && peek(1).kind == Tok::rbracket) {
          next();
          next();
          Expr *s = node(ExprKind::slice, loc);
          s->lhs = e;
          e = s;
          continue;
        }
        Expr *first = expr();
        if (!first) return nullptr;
        if (at(Tok::dotdotlt)) {
          next();
          Expr *s = node(ExprKind::slice, loc);
          s->lhs = e;
          s->rhs = first;
          if (!(s->third = expr()) || !expect(Tok::rbracket, "to close the slice")) return nullptr;
          e = s;
        } else {
          if (at(Tok::dotdot)) {
            fail(peek().loc, "E0216", "ranges are half-open: write '..<'", "a[lo ..< hi]");
            return nullptr;
          }
          Expr *ix = node(ExprKind::index, loc);
          ix->lhs = e;
          ix->rhs = first;
          if (!expect(Tok::rbracket, "to close the index")) return nullptr;
          e = ix;
        }
      } else {
        break;
      }
    }
    return e;
  }

  Expr *call_args(Expr *e) {
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

  Expr *primary() {
    const Token &t = peek();
    Expr *e = node(ExprKind::int_lit, t.loc);
    switch (t.kind) {
      case Tok::int_lit:
        e->int_value = t.int_value;
        next();
        return e;
      case Tok::float_lit:
        e->kind = ExprKind::float_lit;
        e->float_value = t.float_value;
        if (t.float_bits) {
          e->type = t.float_bits == 32 ? &T_F32 : &T_F64;
          if (t.float_bits == 32) e->float_value = (double)(float)t.float_value;
        }
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
      case Tok::lbracket: {
        // [N] T.[a, b, c]
        e->kind = ExprKind::array_lit;
        if (!(e->type_expr = type())) return nullptr;
        if (e->type_expr->kind != TypeExpr::array) {
          fail(t.loc, "E0217", "an array literal needs a fixed array type", "[3] int.[1, 2, 3]");
          return nullptr;
        }
        if (!expect(Tok::dot, "after the array type") || !expect(Tok::lbracket, "to start the elements"))
          return nullptr;
        std::vector<Expr *> elems;
        while (!at(Tok::rbracket) && !failed) {
          Expr *a = expr();
          if (!a) return nullptr;
          elems.push_back(a);
          if (!at(Tok::comma)) break;
          next();
        }
        if (!expect(Tok::rbracket, "to close the array literal")) return nullptr;
        e->args = to_slice(arena, elems);
        return e;
      }
      case Tok::keyword:
        if (str_eq(t.text, "true") || str_eq(t.text, "false")) {
          e->kind = ExprKind::bool_lit;
          e->int_value = str_eq(t.text, "true");
          next();
          return e;
        }
        if (str_eq(t.text, "ifx")) {
          // ifx condition then a else b
          e->kind = ExprKind::ifx;
          next();
          if (!(e->lhs = expr()) || !expect_keyword("then", "after the ifx condition") || !(e->rhs = expr()) ||
              !expect_keyword("else", "in ifx; both branches are required") || !(e->third = expr()))
            return nullptr;
          return e;
        }
        if (str_eq(t.text, "size_of")) {
          // size_of(T): the host C size, in bytes, as an int.
          e->kind = ExprKind::call;
          e->text = t.text;
          next();
          if (!expect(Tok::lparen, "after size_of") || !(e->type_expr = type()) ||
              !expect(Tok::rparen, "after the type"))
            return nullptr;
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
        return call_args(e);
      }
      default:
        fail(t.loc, "E0205", std::string("expected an expression, found ") + tok_name(t.kind));
        return nullptr;
    }
  }

  // ---- statements ----

  Stmt *new_stmt(StmtKind kind, Loc loc) {
    Stmt *s = make<Stmt>(arena);
    s->kind = kind;
    s->loc = loc;
    return s;
  }

  Stmt *block() {
    Stmt *b = new_stmt(StmtKind::block, peek().loc);
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
    Stmt *s = new_stmt(StmtKind::if_, next().loc);  // 'if'
    if (!(s->expr = expr())) return nullptr;
    if (!(s->then_ = block())) return nullptr;
    if (at_keyword("else")) {
      next();
      s->else_ = at_keyword("if") ? if_stmt() : block();
      if (!s->else_) return nullptr;
    }
    return s;
  }

  // for i: lo ..< hi { }   for item: values { }   for item, index: values { }
  Stmt *for_stmt() {
    Stmt *s = new_stmt(StmtKind::for_each, next().loc);
    Loc loc;
    if (!ident(&s->name, &loc, "a loop variable name")) return nullptr;
    if (at(Tok::comma)) {
      next();
      if (!ident(&s->name2, &s->name2_loc, "an index variable name")) return nullptr;
    }
    if (!expect(Tok::colon, "after the loop variable")) return nullptr;
    if (!(s->expr = expr())) return nullptr;
    if (at(Tok::dotdotlt)) {
      next();
      s->kind = StmtKind::for_range;
      if (s->name2.count) {
        fail(s->name2_loc, "E0218", "a range loop has one variable", "for i: 0..<n { }");
        return nullptr;
      }
      if (!(s->expr2 = expr())) return nullptr;
    } else if (at(Tok::dotdot)) {
      fail(peek().loc, "E0216", "ranges are half-open: write '..<'", "for i: 0..<n { }");
      return nullptr;
    }
    if (!(s->then_ = block())) return nullptr;
    return s;
  }

  static bool is_assign_op(Tok k) {
    switch (k) {
      case Tok::equal: case Tok::plus_equal: case Tok::minus_equal: case Tok::star_equal:
      case Tok::slash_equal: case Tok::percent_equal: case Tok::amp_equal: case Tok::pipe_equal:
      case Tok::caret_equal: case Tok::shl_equal: case Tok::shr_equal:
        return true;
      default: return false;
    }
  }

  // An assignment or expression statement, without the ';' (defer reuses it).
  Stmt *simple_stmt() {
    Expr *e = expr();
    if (!e) return nullptr;
    Tok k = peek().kind;
    if (is_assign_op(k)) {
      if (e->kind != ExprKind::ident && e->kind != ExprKind::field && e->kind != ExprKind::index) {
        fail(e->loc, "E0207", "only a variable, a field or an element can be assigned to");
        return nullptr;
      }
      Stmt *s = new_stmt(StmtKind::assign, next().loc);
      s->op = k;
      s->target = e;
      if (!(s->expr = expr())) return nullptr;
      return s;
    }
    Stmt *s = new_stmt(StmtKind::expr, e->loc);
    s->expr = e;
    return s;
  }

  Stmt *stmt() {
    const Token &t = peek();
    if (t.kind == Tok::lbrace) return block();
    if (t.kind == Tok::directive && str_eq(t.text, "assert")) {
      // #assert condition;  checked when it runs, in every build
      Stmt *s = new_stmt(StmtKind::assert_, next().loc);
      if (!(s->expr = expr()) || !expect(Tok::semicolon, "after #assert")) return nullptr;
      return s;
    }
    if (t.kind == Tok::keyword) {
      if (str_eq(t.text, "if")) return if_stmt();
      if (str_eq(t.text, "for")) return for_stmt();
      if (str_eq(t.text, "while")) {
        Stmt *s = new_stmt(StmtKind::while_, next().loc);
        if (!(s->expr = expr())) return nullptr;
        if (!(s->then_ = block())) return nullptr;
        return s;
      }
      if (str_eq(t.text, "return")) {
        Stmt *s = new_stmt(StmtKind::ret, next().loc);
        if (!at(Tok::semicolon) && !(s->expr = expr())) return nullptr;
        if (!expect(Tok::semicolon, "after the return")) return nullptr;
        return s;
      }
      if (str_eq(t.text, "break") || str_eq(t.text, "continue")) {
        Stmt *s = new_stmt(str_eq(t.text, "break") ? StmtKind::break_ : StmtKind::continue_, next().loc);
        if (!expect(Tok::semicolon, str_eq(t.text, "break") ? "after break" : "after continue")) return nullptr;
        return s;
      }
      if (str_eq(t.text, "defer")) {
        // defer { ... }  or  defer statement;
        Stmt *s = new_stmt(StmtKind::defer, next().loc);
        if (at(Tok::lbrace)) {
          if (!(s->then_ = block())) return nullptr;
        } else {
          if (!(s->then_ = simple_stmt()) || !expect(Tok::semicolon, "after the deferred statement")) return nullptr;
        }
        return s;
      }
    }

    // Declarations: name := expr;  name: type [= expr | = ---];
    if (t.kind == Tok::ident && (peek(1).kind == Tok::colonequal || peek(1).kind == Tok::colon)) {
      Stmt *s = new_stmt(StmtKind::var, t.loc);
      s->name = t.text;
      next();
      if (next().kind == Tok::colon) {
        if (!(s->type_expr = type())) return nullptr;
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
      fail(peek(1).loc, "E0206", "local constants, procedures and types are not supported by coax0 yet",
           "move it to the top level");
      return nullptr;
    }

    Stmt *s = simple_stmt();
    if (!s) return nullptr;
    if (!expect(Tok::semicolon, s->kind == StmtKind::assign ? "after the assignment" : "after the statement"))
      return nullptr;
    return s;
  }

  // ---- top level ----

  bool starts_procedure() const {
    if (!at(Tok::lparen)) return false;
    int depth = 0;
    for (int i = 0; peek(i).kind != Tok::eof; i++) {
      if (peek(i).kind == Tok::lparen) depth++;
      if (peek(i).kind == Tok::rparen && --depth == 0) {
        const Token &after = peek(i + 1);
        return after.kind == Tok::lbrace || after.kind == Tok::arrow || after.kind == Tok::directive;
      }
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
      if (!ident(&prm.name, &prm.loc, "a parameter name")) return nullptr;
      if (!expect(Tok::colon, "after the parameter name")) return nullptr;
      if (!(prm.type_expr = type())) return nullptr;
      params.push_back(prm);
      if (!at(Tok::comma)) break;
      next();
    }
    if (!expect(Tok::rparen, "to close the parameter list")) return nullptr;
    if (at(Tok::arrow)) {
      next();
      if (!(p->ret_expr = type())) return nullptr;
    }
    while (at(Tok::directive)) {
      const Token &d = peek();
      if (str_eq(d.text, "export_c")) {
        p->export_c = true;
        next();
      } else if (str_eq(d.text, "foreign")) {
        // name :: (params) -> T #foreign;  a C symbol the host links in.
        p->foreign = true;
        next();
        if (p->export_c) {
          fail(d.loc, "E0210", "a procedure is either #foreign or #export_c, not both");
          return nullptr;
        }
        if (!expect(Tok::semicolon, "after #foreign; a foreign procedure has no body")) return nullptr;
        p->params = to_slice(arena, params);
        return p;
      } else {
        fail(d.loc, "E0210", "#" + to_std(d.text) + " is not supported on procedures by coax0 yet");
        return nullptr;
      }
    }
    p->params = to_slice(arena, params);
    if (!(p->body = block())) return nullptr;
    return p;
  }

  // Name :: struct { field: T; field: T = default; }
  StructDecl *struct_decl(const Token &name) {
    StructDecl *d = make<StructDecl>(arena);
    d->name = name.text;
    d->loc = name.loc;
    next();  // 'struct'
    if (at(Tok::directive)) {
      fail(peek().loc, "E0210", "#" + to_std(peek().text) + " on structs is not supported by coax0 yet");
      return nullptr;
    }
    if (!expect(Tok::lbrace, "to start the struct's fields")) return nullptr;
    std::vector<Field> fields;
    while (!at(Tok::rbrace) && !failed) {
      Field f;
      if (at_keyword("using")) {
        fail(peek().loc, "E0204", "'using' is reserved and not supported by coax0 yet");
        return nullptr;
      }
      if (!ident(&f.name, &f.loc, "a field name") || !expect(Tok::colon, "after the field name") ||
          !(f.type_expr = type()))
        return nullptr;
      if (at(Tok::equal)) {
        next();
        if (!(f.init = expr())) return nullptr;
      }
      if (!expect(Tok::semicolon, "after the field")) return nullptr;
      fields.push_back(f);
    }
    if (!expect(Tok::rbrace, "to close the struct")) return nullptr;
    d->fields = to_slice(arena, fields);
    return d;
  }

  // Name :: enum u8 { member; member = value; }
  EnumDecl *enum_decl(const Token &name) {
    EnumDecl *d = make<EnumDecl>(arena);
    d->name = name.text;
    d->loc = name.loc;
    next();  // 'enum'
    if (at(Tok::lbrace)) {
      fail(peek().loc, "E0219", "an enum names its underlying integer type", "Outcome :: enum u8 { ... }");
      return nullptr;
    }
    if (!(d->base_expr = type()) || !expect(Tok::lbrace, "to start the enum's members")) return nullptr;
    std::vector<EnumMember> members;
    while (!at(Tok::rbrace) && !failed) {
      EnumMember m;
      if (!ident(&m.name, &m.loc, "an enum member name")) return nullptr;
      if (at(Tok::equal)) {
        next();
        if (!(m.value = expr())) return nullptr;
      }
      if (!expect(Tok::semicolon, "after the enum member")) return nullptr;
      members.push_back(m);
    }
    if (!expect(Tok::rbrace, "to close the enum")) return nullptr;
    d->members = to_slice(arena, members);
    return d;
  }

  // #test "name" { ... }
  // #forall ["name"] a: u8, b: lo..<hi { ... }
  CheckDecl *check_decl(const Token &d) {
    CheckDecl *c = make<CheckDecl>(arena);
    c->loc = d.loc;
    c->forall = str_eq(d.text, "forall");
    ProcDecl *p = make<ProcDecl>(arena);
    p->loc = d.loc;
    c->proc = p;
    if (at(Tok::string_lit)) {
      c->name = next().string_value;
    } else if (!c->forall) {
      fail(peek().loc, "E0220", "#test needs a name string", "#test \"refunds never exceed payment\" { ... }");
      return nullptr;
    }
    std::vector<Param> params;
    std::vector<Binding> domains;
    int32_t start = peek().loc.offset;
    while (c->forall && !at(Tok::lbrace) && !failed) {
      Param prm;
      Binding b;
      if (!ident(&prm.name, &prm.loc, "a binding name")) return nullptr;
      if (!expect(Tok::colon, "after the binding name")) return nullptr;
      if (peek(1).kind == Tok::comma || peek(1).kind == Tok::lbrace) {
        if (!(prm.type_expr = type())) return nullptr;
      } else {
        if (!(b.lo = expr())) return nullptr;
        if (!expect(Tok::dotdotlt, "in the range; a #forall binding is a type or lo..<hi")) return nullptr;
        if (!(b.hi = expr())) return nullptr;
      }
      params.push_back(prm);
      domains.push_back(b);
      if (!at(Tok::comma)) break;
      next();
    }
    if (c->forall && params.empty()) {
      fail(peek().loc, "E0220", "#forall needs at least one binding", "#forall a: u8, b: bool { ... }");
      return nullptr;
    }
    if (c->forall && !c->name.count) {
      int32_t end = peek().loc.offset;
      while (end > start && (src->text[(size_t)end - 1] == ' ' || src->text[(size_t)end - 1] == '\n')) end--;
      c->name = Str{src->text.data() + start, end - start};
    }
    p->name = c->name;
    p->params = to_slice(arena, params);
    c->domains = to_slice(arena, domains);
    if (!(p->body = block())) return nullptr;
    return c;
  }

  bool file(File *out) {
    std::vector<Import> imports;
    std::vector<ProcDecl *> procs;
    std::vector<ConstDecl *> constants;
    std::vector<StructDecl *> structs;
    std::vector<EnumDecl *> enums;
    std::vector<Expr *> assertions;
    std::vector<CheckDecl *> checks;
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
      if (t.kind == Tok::directive && (str_eq(t.text, "test") || str_eq(t.text, "forall"))) {
        Token d = next();
        CheckDecl *c = check_decl(d);
        if (!c) break;
        checks.push_back(c);
        continue;
      }
      if (t.kind == Tok::directive && str_eq(t.text, "authority")) {
        next();
        out->authority = true;
        if (!expect(Tok::semicolon, "after #authority")) break;
        continue;
      }
      if (t.kind == Tok::directive) {
        fail(t.loc, "E0210", "#" + to_std(t.text) + " is not supported by coax0 yet");
        break;
      }
      if (t.kind == Tok::ident && peek(1).kind == Tok::coloncolon) {
        next();
        next();
        if (at_keyword("struct")) {
          StructDecl *d = struct_decl(t);
          if (!d) break;
          structs.push_back(d);
        } else if (at_keyword("enum")) {
          EnumDecl *d = enum_decl(t);
          if (!d) break;
          enums.push_back(d);
        } else if (at_keyword("union") || at_keyword("enum_flags")) {
          fail(peek().loc, "E0204", "'" + to_std(peek().text) + "' is reserved and not supported by coax0 yet");
          break;
        } else if (starts_procedure()) {
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
    out->structs = to_slice(arena, structs);
    out->enums = to_slice(arena, enums);
    out->assertions = to_slice(arena, assertions);
    out->checks = to_slice(arena, checks);
    return !failed;
  }
};

}  // namespace

bool parse(Arena *arena, const Source *src, const std::vector<Token> &tokens, File *out) {
  Parser p{arena, src, &tokens};
  return p.file(out);
}

}  // namespace coax
