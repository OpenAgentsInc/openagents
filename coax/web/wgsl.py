#!/usr/bin/env python3
# WGSL from Coax, for now: `coax metal` emits a scalar f32/bool Metal library
# (straight-line C: locals, if/else, return, ?: choices, and comma sequences
# that assign temporaries), and this rewrites the functions a page names,
# with everything they call, as WGSL. Coaxial pages then call the same light
# functions the iOS stages do (verse/light.coax). Until Coax emits WGSL
# itself (coax/web.md); anything outside that shape is refused, not guessed.
#
#   wgsl.py FILE.coax NAME...            prints the WGSL
#   wgsl.py FILE.coax NAME... --into F   replaces F's block between
#                                        "// ---- generated: FILE.coax" and "// ---- end generated"
import os, re, subprocess, sys

def metal(coax_file):
    repo = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    return subprocess.run([os.path.join(repo, "coax/bin/coax"), "metal", coax_file],
                          check=True, capture_output=True, text=True).stdout

def functions(src):
    """name -> (signature params, return type, body lines) for each defined function."""
    out = {}
    lines = src.split("\n")
    i = 0
    head = re.compile(r"^(?:static )?(float|bool) (\w+)\((.*)\) \{$")
    while i < len(lines):
        m = head.match(lines[i])
        if not m:
            i += 1; continue
        ret, name, params = m.groups()
        body = []; i += 1
        while lines[i] != "}":
            body.append(lines[i]); i += 1
        out[name] = (params, ret, body)
        i += 1
    return out

TOKEN = re.compile(r"\s*(?:(\d+\.\d*(?:e[-+]?\d+)?f?|\d+f?)|(\w+)|(==|!=|<=|>=|&&|\|\||[-+*/<>=!(),?:]))")

def tokens(text):
    pos, out = 0, []
    text = text.rstrip()
    while pos < len(text):
        m = TOKEN.match(text, pos)
        if not m or m.end() == pos:
            raise SystemExit(f"wgsl.py: can't read {text[pos:pos + 30]!r}")
        num, word, op = m.groups()
        out.append(("num", num) if num else ("id", word) if word else ("op", op))
        pos = m.end()
    return out

class Expr:
    """Parses one C expression; comma sequences and assignments to
    temporaries become statements hoisted before the one using them."""
    squares = 0
    PREC = [["||"], ["&&"], ["==", "!="], ["<", ">", "<=", ">="], ["+", "-"], ["*", "/"]]
    def __init__(self, toks, hoist):
        self.t, self.i, self.hoist = toks, 0, hoist
    def peek(self): return self.t[self.i] if self.i < len(self.t) else ("end", "")
    def take(self, v=None):
        k = self.peek()
        if v is not None and k[1] != v: raise SystemExit(f"wgsl.py: expected {v!r}, found {k[1]!r}")
        self.i += 1; return k
    def sequence(self):
        e = self.assign()
        while self.peek() == ("op", ","):
            self.take(",")   # what came before only assigned temporaries, now hoisted
            e = self.assign()
        return e
    def assign(self):
        start = self.i
        if self.peek()[0] == "id" and self.i + 1 < len(self.t) and self.t[self.i + 1] == ("op", "="):
            name = self.take()[1]; self.take("=")
            value = self.assign()
            self.hoist.append(f"{name} = {value};")
            return name
        self.i = start
        return self.choice()
    def choice(self):
        # c ? a : b. WGSL's select evaluates both; these are pure scalar math.
        c = self.binary(0)
        if self.peek() != ("op", "?"): return c
        self.take("?"); a = self.choice(); self.take(":"); b = self.choice()
        return f"select({b}, {a}, {c})"
    def binary(self, level):
        if level == len(self.PREC): return self.unary()
        e = self.binary(level + 1)
        while self.peek()[0] == "op" and self.peek()[1] in self.PREC[level]:
            op = self.take()[1]
            e = f"({e} {op} {self.binary(level + 1)})"
        return e
    def unary(self):
        if self.peek() in (("op", "-"), ("op", "!")):
            op = self.take()[1]; return f"({op}{self.unary()})"
        return self.primary()
    def primary(self):
        kind, v = self.take()
        if kind == "num":
            v = v.rstrip("f")
            return v if "." in v or "e" in v else v + ".0"
        if kind == "op" and v == "(":
            e = self.sequence(); self.take(")"); return e
        if kind == "id":
            if v in ("true", "false"): return v
            if self.peek() == ("op", "("):
                self.take("("); args = []
                if self.peek() != ("op", ")"):
                    args.append(self.assign())
                    while self.peek() == ("op", ","):
                        self.take(","); args.append(self.assign())
                self.take(")")
                if v == "pow" and args[1] == "2.0":
                    # WGSL's pow is undefined below zero; a square is a product
                    Expr.squares += 1; t = f"coax_sq{Expr.squares}"
                    self.hoist.append(f"let {t}: f32 = {args[0]};")
                    return f"({t} * {t})"
                return f"{v}({', '.join(args)})"
            return v
        raise SystemExit(f"wgsl.py: unexpected {v!r}")

def expr(text, hoist):
    p = Expr(tokens(text), hoist)
    e = p.sequence()
    if p.i != len(p.t): raise SystemExit(f"wgsl.py: left over in {text!r}")
    return e

WGSL_TYPE = {"float": "f32", "bool": "bool"}

def convert(name, params, ret, body):
    names = []
    args = []
    for p in filter(None, (x.strip() for x in params.split(","))):
        t, n = p.split(); args.append(f"{n}: {WGSL_TYPE[t]}"); names.append(n)
    out = [f"fn {name}({', '.join(args)}) -> {WGSL_TYPE[ret]} {{"]
    for line in body:
        indent = line[:len(line) - len(line.lstrip())]
        s = line.strip()
        hoist = []
        if s in ("{", "}", "else"):
            out.append(indent + s); continue
        if m := re.match(r"^(float|bool) (\w+);$", s):
            out.append(f"{indent}var {m[2]}: {WGSL_TYPE[m[1]]};"); continue
        if m := re.match(r"^(float|bool) (\w+) = (.*);$", s):
            e = expr(m[3], hoist); stmt = f"var {m[2]}: {WGSL_TYPE[m[1]]} = {e};"
        elif m := re.match(r"^return (.*);$", s):
            e = expr(m[1], hoist); stmt = f"return {e};"
        elif m := re.match(r"^if \((.*)\)$", s):
            e = expr(m[1], hoist); stmt = f"if ({e})"
        elif m := re.match(r"^(\w+) = (.*);$", s):
            if m[1] in names: raise SystemExit(f"wgsl.py: {name} assigns its parameter {m[1]}")
            e = expr(m[2], hoist); stmt = f"{m[1]} = {e};"
        else:
            raise SystemExit(f"wgsl.py: {name}: can't convert {s!r}")
        out += [indent + h for h in hoist]
        out.append(indent + stmt)
    out.append("}")
    return out

def closure(fns, roots):
    seen, order = set(), []
    def visit(n):
        if n in seen: return
        if n not in fns: raise SystemExit(f"wgsl.py: no function {n}")
        seen.add(n)
        for c in re.findall(r"\b(\w+)\(", "\n".join(fns[n][2])):
            if c in fns: visit(c)
        order.append(n)
    for r in roots: visit(r)
    return order

def main():
    args = sys.argv[1:]
    into = None
    if "--into" in args:
        k = args.index("--into"); into = args[k + 1]; del args[k:k + 2]
    coax_file, roots = args[0], args[1:]
    fns = functions(metal(coax_file))
    lines = [f"// ---- generated: {coax_file} ({', '.join(roots)}) by coax/web/wgsl.py; do not edit"]
    for n in closure(fns, roots):
        lines += convert(n, *fns[n])
    lines.append("// ---- end generated")
    text = "\n".join(lines) + "\n"
    if not into:
        sys.stdout.write(text); return
    page = open(into).read()
    start = page.find(f"// ---- generated: {coax_file}")
    end = page.find("// ---- end generated\n", start)
    if start < 0 or end < 0: raise SystemExit(f"wgsl.py: {into} has no generated block for {coax_file}")
    new = page[:start] + text + page[end + len("// ---- end generated\n"):]
    if new != page:
        open(into + ".tmp", "w").write(new); os.replace(into + ".tmp", into)

main()
