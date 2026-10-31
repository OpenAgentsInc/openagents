#!/usr/bin/env python3
"""Struct defaults for the browser, as native applies them.

Natively `s: [1] T` runs T's field defaults (`spin_g: f64 = 1.0`); in the
browser the host hands the engine zeroed memory and a face's `static coax_t_T x;`
is zeroed too, so every default read as 0 (Orbit League ran without gravity,
2026-10-31). The compiler already emits `static T coax_init_t_T(void)` for
each struct with a default. This gives the ones a face needs a public name
(appended to the unit that has it) and writes the face's wrapper unit:
  - export coaxial_state_init(state): the host calls it on a fresh state;
  - export coaxial_state_default(i): where the i-th defaulted part of the
    state sits (-1 past the end), for state-defaults.test.ts;
  - a constructor, run by _initialize, defaults the face's coax globals.

  defaults.py FACE.c WRAPPER.c UNIT.c...
  defaults.py --plan FACE.c UNIT.c...   (JSON: what it would default)
"""
import json
import re
import sys


def plan(face, units):
    defaulted = {}  # type -> unit with its coax_init
    for unit in units:
        for t in re.findall(r"^static (coax_t_\w+) coax_init_\1\(void\)", open(unit).read().replace("coax_init_t_", "coax_init_coax_t_"), re.M):
            defaulted.setdefault(t, unit)
    # The state: sizeof(coax_t_T), or a face struct of coax members.
    state, name = [], None  # [(member or "", type)]
    size = re.search(r"\"coaxial_state_size\"\)[^{]*\{[^}]*?sizeof\s*\(?\s*(\w+)", face)
    if size:
        name = size.group(1)
        if name.startswith("coax_t_"):
            state.append(("", name))
        else:
            typedef = re.search(r"typedef struct\s*\{([^}]*)\}\s*" + name + r"\s*;", face)
            if typedef:
                state += [(m, t) for t, m in re.findall(r"(coax_t_\w+)\s+(\w+)\s*;", typedef.group(1))]
    state = [(m, t) for m, t in state if t in defaulted]
    # The face's file-scope coax globals: static coax_t_T name; or name[N];
    globals_ = [(t, n, a) for t, n, a in re.findall(r"^static (coax_t_\w+) (\w+)(\[[^\]]+\])?;", face, re.M)
                if t in defaulted]
    needed = sorted({t for _, t in state} | {t for t, _, _ in globals_})
    return dict(name=name, state=state, globals=globals_, units={t: defaulted[t] for t in needed},
                own_init="coaxial_state_init" in face)


if sys.argv[1] == "--plan":
    face_path, units = sys.argv[2], sys.argv[3:]
    print(json.dumps(plan(open(face_path).read(), units)))
    sys.exit(0)

face_path, wrapper_path, units = sys.argv[1], sys.argv[2], sys.argv[3:]
p = plan(open(face_path).read(), units)
by_unit = {}
for t, unit in p["units"].items():
    by_unit.setdefault(unit, []).append(t)
INIT = re.compile(r"^static (\w+) coax_init_(\w+)\(void\) \{\n  \1 v;\n(.*?)\n  return v;\n\}\n", re.M | re.S)
NESTED = re.compile(r"\b(v(?:\.\w+|\[[^\]]*\])+) = coax_init_(\w+)\(\);")


def in_place(text):
    """Every coax_init_X of a unit again, writing through p: a state can be
    megabytes (the chat's is 5), past the wasm stack the returned copies need."""
    inits = {suffix: (ctype, body) for ctype, suffix, body in INIT.findall(text)}
    out = [f"static void coaxial_init_{x}({c} *p);\n" for x, (c, _) in inits.items()]
    for suffix, (ctype, body) in inits.items():
        if re.search(r"\bp\b", body):
            out.append(f"static void coaxial_init_{suffix}({ctype} *p) {{ *p = coax_init_{suffix}(); }}\n")
            continue
        body = NESTED.sub(lambda m: f"coaxial_init_{m.group(2)}(&({m.group(1)}));" if m.group(2) in inits else m.group(0), body)
        out.append(f"static void coaxial_init_{suffix}({ctype} *p) {{\n#define v (*p)\n{body}\n#undef v\n}}\n")
    return inits, "".join(out)


for unit, types in by_unit.items():
    inits, code = in_place(open(unit).read())
    with open(unit, "a") as f:
        f.write("\n// defaults.py: the struct defaults the browser face applies.\n" + code)
        for t in types:
            suffix = t.replace("coax_", "", 1)  # coax_t_X -> t_X
            call = f"coaxial_init_{suffix}(p);" if suffix in inits else f"*p = coax_init_{suffix}();"
            f.write(f"void coaxial_default_{t}({t} *p) {{ {call} }}\n")


with open(wrapper_path, "w") as f:
    f.write(f'#include "{face_path}"\n#include <stddef.h>\n')
    for t in p["units"]:
        f.write(f"void coaxial_default_{t}({t} *p);\n")
    if p["globals"]:
        f.write("__attribute__((constructor)) static void coaxial_face_defaults(void) {\n")
        for t, n, a in p["globals"]:
            if a:
                f.write(f"  for (size_t i = 0; i < sizeof {n} / sizeof {n}[0]; i++) coaxial_default_{t}(&{n}[i]);\n")
            else:
                f.write(f"  coaxial_default_{t}(&{n});\n")
        f.write("}\n")
    if p["state"] and not p["own_init"]:
        name = p["name"]
        f.write('__attribute__((export_name("coaxial_state_init"))) void coaxial_state_init(uint32_t state) {\n')
        for m, t in p["state"]:
            at = f"&(({name} *)(uintptr_t)state)->{m}" if m else f"({t} *)(uintptr_t)state"
            f.write(f"  coaxial_default_{t}({at});\n")
        f.write("}\n")
        offsets = ", ".join(f"(int32_t)offsetof({name}, {m})" if m else "0" for m, _ in p["state"])
        f.write('__attribute__((export_name("coaxial_state_default"))) int32_t coaxial_state_default(int32_t i) {\n')
        f.write(f"  static const int32_t at[] = {{{offsets}}};\n")
        f.write(f"  return i >= 0 && i < {len(p['state'])} ? at[i] : -1;\n}}\n")
