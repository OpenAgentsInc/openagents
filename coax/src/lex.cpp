// Lexer: source bytes to tokens. Line comments, nested block comments,
// directives (#name), annotations (@name), and the reserved keywords of
// the spec's §5 (reserved now, even where coax0 does not implement them).
#include "coax.h"

namespace coax {

static const char *KEYWORDS[] = {
    "struct", "union", "enum", "enum_flags", "if", "else", "ifx", "then", "while", "for",
    "break", "continue", "return", "defer", "switch", "case", "using", "cast", "xx", "inline",
    "no_inline", "job", "pure", "strict", "deterministic", "shader", "try", "or_else",
    "push_context", "null", "true", "false", "it", "it_index", "size_of", "type_of",
    "type_info", "Type", "Code", "remove", "constraint",
};

static bool is_keyword(Str s) {
  for (const char *k : KEYWORDS)
    if (str_eq(s, k)) return true;
  return false;
}

const char *tok_name(Tok t) {
  switch (t) {
    case Tok::eof: return "end of file";
    case Tok::ident: return "identifier";
    case Tok::keyword: return "keyword";
    case Tok::int_lit: return "integer";
    case Tok::float_lit: return "float";
    case Tok::string_lit: return "string";
    case Tok::directive: return "directive";
    case Tok::annotation: return "annotation";
    case Tok::lparen: return "'('";
    case Tok::rparen: return "')'";
    case Tok::lbrace: return "'{'";
    case Tok::rbrace: return "'}'";
    case Tok::lbracket: return "'['";
    case Tok::rbracket: return "']'";
    case Tok::comma: return "','";
    case Tok::semicolon: return "';'";
    case Tok::colon: return "':'";
    case Tok::coloncolon: return "'::'";
    case Tok::colonequal: return "':='";
    case Tok::arrow: return "'->'";
    case Tok::dot: return "'.'";
    case Tok::dotdot: return "'..'";
    case Tok::dotdotlt: return "'..<'";
    case Tok::equal: return "'='";
    case Tok::plus_equal: return "'+='";
    case Tok::minus_equal: return "'-='";
    case Tok::star_equal: return "'*='";
    case Tok::slash_equal: return "'/='";
    case Tok::percent_equal: return "'%='";
    case Tok::amp_equal: return "'&='";
    case Tok::pipe_equal: return "'|='";
    case Tok::caret_equal: return "'^='";
    case Tok::shl_equal: return "'<<='";
    case Tok::shr_equal: return "'>>='";
    case Tok::plus: return "'+'";
    case Tok::minus: return "'-'";
    case Tok::star: return "'*'";
    case Tok::slash: return "'/'";
    case Tok::percent: return "'%'";
    case Tok::amp: return "'&'";
    case Tok::pipe: return "'|'";
    case Tok::caret: return "'^'";
    case Tok::shl: return "'<<'";
    case Tok::shr: return "'>>'";
    case Tok::eqeq: return "'=='";
    case Tok::noteq: return "'!='";
    case Tok::lt: return "'<'";
    case Tok::le: return "'<='";
    case Tok::gt: return "'>'";
    case Tok::ge: return "'>='";
    case Tok::andand: return "'&&'";
    case Tok::oror: return "'||'";
    case Tok::bang: return "'!'";
  }
  return "?";
}

static bool is_ident_start(char c) { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'; }
static bool is_digit(char c) { return c >= '0' && c <= '9'; }
static bool is_ident_char(char c) { return is_ident_start(c) || is_digit(c); }

bool lex(Arena *arena, const Source *src, std::vector<Token> *out) {
  const char *s = src->text.data();
  int32_t n = (int32_t)src->text.size();
  int32_t i = 0, line = 1, line_start = 0;
  int32_t errors_before = g_error_count;

  auto loc_at = [&](int32_t at) { return Loc{line, at - line_start + 1, at}; };
  auto newline = [&](int32_t at) { line++; line_start = at + 1; };

  while (i < n) {
    char c = s[i];
    if (c == '\n') { newline(i); i++; continue; }
    if (c == ' ' || c == '\t' || c == '\r') { i++; continue; }

    if (c == '/' && i + 1 < n && s[i + 1] == '/') {
      while (i < n && s[i] != '\n') i++;
      continue;
    }
    if (c == '/' && i + 1 < n && s[i + 1] == '*') {
      Loc start = loc_at(i);
      int32_t depth = 1;
      i += 2;
      while (i < n && depth > 0) {
        if (s[i] == '/' && i + 1 < n && s[i + 1] == '*') { depth++; i += 2; continue; }
        if (s[i] == '*' && i + 1 < n && s[i + 1] == '/') { depth--; i += 2; continue; }
        if (s[i] == '\n') newline(i);
        i++;
      }
      if (depth > 0) error_at(src, start, "E0101", "block comment is never closed", "add '*/'");
      continue;
    }

    Token t;
    t.loc = loc_at(i);
    int32_t start = i;

    if (is_ident_start(c)) {
      while (i < n && is_ident_char(s[i])) i++;
      t.text = Str{s + start, i - start};
      t.kind = is_keyword(t.text) ? Tok::keyword : Tok::ident;
      out->push_back(t);
      continue;
    }

    if ((c == '#' || c == '@') && i + 1 < n && is_ident_start(s[i + 1])) {
      i++;
      int32_t name_start = i;
      while (i < n && is_ident_char(s[i])) i++;
      t.kind = c == '#' ? Tok::directive : Tok::annotation;
      t.text = Str{s + name_start, i - name_start};
      out->push_back(t);
      continue;
    }

    if (is_digit(c)) {
      uint64_t v = 0;
      bool overflow = false;
      int base = 10;
      if (c == '0' && i + 1 < n && (s[i + 1] == 'x' || s[i + 1] == 'b')) {
        base = s[i + 1] == 'x' ? 16 : 2;
        i += 2;
      }
      int32_t digits = 0;
      for (; i < n; i++) {
        char d = s[i];
        int dv;
        if (d == '_') continue;
        if (is_digit(d)) dv = d - '0';
        else if (base == 16 && d >= 'a' && d <= 'f') dv = d - 'a' + 10;
        else if (base == 16 && d >= 'A' && d <= 'F') dv = d - 'A' + 10;
        else break;
        if (dv >= base) break;
        if (v > (UINT64_MAX - (uint64_t)dv) / (uint64_t)base) overflow = true;
        v = v * (uint64_t)base + (uint64_t)dv;
        digits++;
      }
      // A float has a fraction (`1.5`, never `1.` so that `0..<n` stays a
      // range), an exponent, or both. Decimal only.
      bool fraction = base == 10 && i + 1 < n && s[i] == '.' && is_digit(s[i + 1]);
      bool exponent = base == 10 && i < n && (s[i] == 'e' || s[i] == 'E') &&
                      ((i + 1 < n && is_digit(s[i + 1])) ||
                       (i + 2 < n && (s[i + 1] == '-' || s[i + 1] == '+') && is_digit(s[i + 2])));
      if (fraction || exponent) {
        if (fraction) { i++; while (i < n && (is_digit(s[i]) || s[i] == '_')) i++; }
        if (i < n && (s[i] == 'e' || s[i] == 'E')) {
          i++;
          if (i < n && (s[i] == '-' || s[i] == '+')) i++;
          while (i < n && is_digit(s[i])) i++;
        }
        std::string digits_only;
        for (int32_t k = start; k < i; k++)
          if (s[k] != '_') digits_only += s[k];
        t.kind = Tok::float_lit;
        t.float_value = strtod(digits_only.c_str(), nullptr);
        if (t.float_value > 1.7976931348623157e308)
          error_at(src, t.loc, "E0109", "floating-point literal is too large for f64");
        t.text = Str{s + start, i - start};
        out->push_back(t);
        continue;
      }
      if (digits == 0) {
        error_at(src, t.loc, "E0103", "number has no digits after its base prefix");
      } else if (overflow) {
        error_at(src, t.loc, "E0104", "integer literal does not fit in 64 bits");
      }
      t.kind = Tok::int_lit;
      t.int_value = v;
      t.text = Str{s + start, i - start};
      out->push_back(t);
      continue;
    }

    if (c == '\'') {
      // 'x': one ASCII byte, as an untyped integer literal.
      int32_t value = -1;
      if (i + 2 < n && s[i + 1] != '\\' && s[i + 1] != '\'' && s[i + 2] == '\'' && (unsigned char)s[i + 1] < 0x80) {
        value = (unsigned char)s[i + 1];
        i += 3;
      } else if (i + 3 < n && s[i + 1] == '\\' && s[i + 3] == '\'') {
        switch (s[i + 2]) {
          case 'n': value = '\n'; break;
          case 't': value = '\t'; break;
          case 'r': value = '\r'; break;
          case '0': value = 0; break;
          case '\\': value = '\\'; break;
          case '\'': value = '\''; break;
          default: break;
        }
        i += 4;
      }
      if (value < 0) {
        error_at(src, t.loc, "E0110", "a character literal is one ASCII character in single quotes",
                 "write 'a', or an escape such as '\\n'; text belongs in \"strings\"");
        i++;
        while (i < n && s[i] != '\'' && s[i] != '\n') i++;
        if (i < n && s[i] == '\'') i++;
        continue;
      }
      t.kind = Tok::int_lit;
      t.int_value = (uint64_t)value;
      t.text = Str{s + start, i - start};
      out->push_back(t);
      continue;
    }

    if (c == '"') {
      i++;
      std::string value;
      bool closed = false;
      while (i < n) {
        char d = s[i];
        if (d == '"') { closed = true; i++; break; }
        if (d == '\n') break;
        if (d == '\\' && i + 1 < n) {
          char e = s[i + 1];
          i += 2;
          switch (e) {
            case 'n': value += '\n'; break;
            case 't': value += '\t'; break;
            case 'r': value += '\r'; break;
            case '0': value += '\0'; break;
            case '\\': value += '\\'; break;
            case '"': value += '"'; break;
            default:
              error_at(src, loc_at(i - 2), "E0105", std::string("unknown escape '\\") + e + "'",
                       "use one of \\n \\t \\r \\0 \\\\ \\\"");
          }
          continue;
        }
        value += d;
        i++;
      }
      if (!closed) error_at(src, t.loc, "E0106", "string is never closed", "add '\"' before the end of the line");
      char *buf = (char *)arena_alloc(arena, value.size() + 1);
      memcpy(buf, value.data(), value.size());
      buf[value.size()] = 0;
      t.kind = Tok::string_lit;
      t.string_value = Str{buf, (int32_t)value.size()};
      t.text = Str{s + start, i - start};
      out->push_back(t);
      continue;
    }

    auto two = [&](char a, char b) { return c == a && i + 1 < n && s[i + 1] == b; };
    Tok k = Tok::eof;
    int32_t len = 1;
    auto three = [&](char a, char b, char d) { return c == a && i + 2 < n && s[i + 1] == b && s[i + 2] == d; };
    if (three('.', '.', '<')) { k = Tok::dotdotlt; len = 3; }
    else if (three('<', '<', '=')) { k = Tok::shl_equal; len = 3; }
    else if (three('>', '>', '=')) { k = Tok::shr_equal; len = 3; }
    else if (two('<', '<')) { k = Tok::shl; len = 2; }
    else if (two('>', '>')) { k = Tok::shr; len = 2; }
    else if (two('&', '=')) { k = Tok::amp_equal; len = 2; }
    else if (two('|', '=')) { k = Tok::pipe_equal; len = 2; }
    else if (two('^', '=')) { k = Tok::caret_equal; len = 2; }
    else if (two(':', ':')) { k = Tok::coloncolon; len = 2; }
    else if (two(':', '=')) { k = Tok::colonequal; len = 2; }
    else if (two('-', '>')) { k = Tok::arrow; len = 2; }
    else if (two('.', '.')) { k = Tok::dotdot; len = 2; }
    else if (two('=', '=')) { k = Tok::eqeq; len = 2; }
    else if (two('!', '=')) { k = Tok::noteq; len = 2; }
    else if (two('<', '=')) { k = Tok::le; len = 2; }
    else if (two('>', '=')) { k = Tok::ge; len = 2; }
    else if (two('&', '&')) { k = Tok::andand; len = 2; }
    else if (two('|', '|')) { k = Tok::oror; len = 2; }
    else if (two('+', '=')) { k = Tok::plus_equal; len = 2; }
    else if (two('-', '=')) { k = Tok::minus_equal; len = 2; }
    else if (two('*', '=')) { k = Tok::star_equal; len = 2; }
    else if (two('/', '=')) { k = Tok::slash_equal; len = 2; }
    else if (two('%', '=')) { k = Tok::percent_equal; len = 2; }
    else switch (c) {
      case '(': k = Tok::lparen; break;
      case ')': k = Tok::rparen; break;
      case '{': k = Tok::lbrace; break;
      case '}': k = Tok::rbrace; break;
      case '[': k = Tok::lbracket; break;
      case ']': k = Tok::rbracket; break;
      case ',': k = Tok::comma; break;
      case ';': k = Tok::semicolon; break;
      case ':': k = Tok::colon; break;
      case '.': k = Tok::dot; break;
      case '=': k = Tok::equal; break;
      case '+': k = Tok::plus; break;
      case '-': k = Tok::minus; break;
      case '*': k = Tok::star; break;
      case '/': k = Tok::slash; break;
      case '%': k = Tok::percent; break;
      case '<': k = Tok::lt; break;
      case '>': k = Tok::gt; break;
      case '!': k = Tok::bang; break;
      case '&': k = Tok::amp; break;
      case '|': k = Tok::pipe; break;
      case '^': k = Tok::caret; break;
      default: break;
    }
    if (k == Tok::eof) {
      if ((unsigned char)c >= 0x80)
        error_at(src, t.loc, "E0107", "non-ASCII character outside a string or comment",
                 "identifiers are ASCII only (spec §5)");
      else
        error_at(src, t.loc, "E0108", std::string("unexpected character '") + c + "'");
      i++;
      continue;
    }
    t.kind = k;
    t.text = Str{s + i, len};
    i += len;
    out->push_back(t);
  }

  Token eof;
  eof.kind = Tok::eof;
  eof.loc = loc_at(n);
  out->push_back(eof);
  return g_error_count == errors_before;
}

}  // namespace coax
