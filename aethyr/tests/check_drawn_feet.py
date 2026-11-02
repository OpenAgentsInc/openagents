#!/usr/bin/env python3
"""Check actual walking-realm render roots through native Coax boundaries.

No browser, models, assets, sound or live service is needed. The probes feed
the production player interpolation into production realm frame functions;
they read the actual figure root/shape records, rather than reimplementing
the space conversion. Python supplies the independently authored expected
coordinates and checks four deliberately broken inputs as negative controls.
"""
import argparse
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
CASES = ("garden", "fairy", "villa", "everglade", "grid", "deadgears", "haunt", "made", "icecream")


def matches(actual, expected):
    return all(math.isfinite(a) and abs(a - e) <= 0.00002 for a, e in zip(actual, expected))


def expected_root(case, origin):
    # Previous feet (1, .25, -6), next feet (5, 1.25, 2), alpha=1/4.
    # This calculation is authored independently of Coax's interpolation.
    x, y, z = 2.0, 0.5, -4.0
    if case == "grid":
        # A Grid drawing unit is two metres, on every axis, once.
        return (origin[0] + x / 2, origin[1] + y / 2, origin[2] + z / 2)
    if case in ("deadgears", "haunt", "made", "icecream"):
        return (x, y, z)  # its player already steps in the ship's world
    return (x, y + 2.5, z)  # authored nonzero flat terrain in the probe


def run(cmd, *, stdout=subprocess.PIPE):
    result = subprocess.run(cmd, cwd=ROOT, stdout=stdout, stderr=subprocess.PIPE, timeout=180)
    if result.returncode:
        raise RuntimeError(f"{' '.join(map(str, cmd))}\n{result.stderr.decode(errors='replace')[-3000:]}")
    return result.stdout


def check(case, compiler, directory, cc, release=False, sanitize=False):
    codegen = ["--release"] if release else []
    source = ROOT / "aethyr/tests" / f"feet-{case}.coax"
    code, header, driver, binary = (directory / f"{case}{suffix}" for suffix in (".c", ".h", "-main.c", "-native"))
    with code.open("wb") as output:
        run([compiler, "c", str(source), "--library", "--header", str(header), *codegen], stdout=output)
    player_c, player_h = directory / "Player.c", directory / "Player.h"
    if not player_c.exists():
        with player_c.open("wb") as output:
            run([compiler, "c", str(ROOT / "aethyr/player.coax"), "--library", "--header", str(player_h), *codegen], stdout=output)
    # Ice Cream Man and the shared controller both call their distinct
    # struct Player. Keep those C headers in separate translation units,
    # just as the product's independent libraries keep their ABIs apart.
    input_c = directory / "input.c"
    input_c.write_text(f'''#include "{player_h.name}"
#include <stdbool.h>
void probe_input(int mode, float result[3]) {{
  coax_t_Player p = verse_player_place(5, 1.25f, 2, 0);
  p.prev_pos = (coax_t_Vec3){{1, 0.25f, -6}};
  p.clock.carry_ns = p.clock.step_ns / 4;
  p.on_ground = false;
  if (mode == 1) {{ p.prev_pos.y = 0; p.pos.y = 0; }}
  if (mode == 2) {{ p.prev_pos.y *= 2; p.pos.y *= 2; }}
  if (mode == 3) p.prev_pos = p.pos;
  if (mode == 4) {{ p.prev_pos.y += 2.5f; p.pos.y += 2.5f; }}
  coax_t_Vec3 feet = verse_player_feet(p);
  result[0] = feet.x; result[1] = feet.y; result[2] = feet.z;
}}
''')
    # Every realm links the facade for probe_input. Dead Gears imports the
    # shared Walk core directly; that core intentionally exports no C symbols.
    libraries = [str(input_c), str(player_c)]
    if case == "deadgears":
        # Both libraries also emit Input's C helper. Give the facade's copy
        # a private test-link name without changing either generated body.
        player_unit = directory / "Player-link.c"
        player_unit.write_text('#define input_runs drawn_feet_player_input_runs\n#include "Player.c"\n')
        libraries[-1] = str(player_unit)
    extra_headers = ""
    figure_check = ""
    if case == "villa":
        figure_c, figure_h = directory / "Villafig.c", directory / "Villafig.h"
        with figure_c.open("wb") as output:
            run([compiler, "c", str(ROOT / "apps/openagents-web/villafig.coax"), "--library", "--header", str(figure_h), *codegen], stdout=output)
        libraries.append(str(figure_c))
        # The production face deliberately keeps Figs opaque: its Place
        # and the realm's saved Place have different public C layouts.
        extra_headers = '''typedef struct { void *data; int64_t count; } figs_view;
void vf_start(figs_view);
int64_t vf_size(void);
int64_t vf_place(figs_view, int64_t, float, float, float, float, coax_v_f32, coax_v_f32);
'''
        figure_check = '''
    void *figures = calloc(1, (size_t)vf_size());
    if (!figures) return (void *)1;
    vf_start((figs_view){figures, 1});
    float baseline[4096] = {0}, bones[4096] = {0}, bound[4] = {0};
    int64_t base_n = vf_place((figs_view){figures, 1}, 0, 0, 0, 0, 0,
      (coax_v_f32){baseline, 4096}, (coax_v_f32){bound, 4});
    int64_t n = vf_place((figs_view){figures, 1}, 0, values[0], values[1], values[2], 0,
      (coax_v_f32){bones, 4096}, (coax_v_f32){bound, 4});
    for (int axis = 0; axis < 3; ++axis) values[axis] = bones[axis] - baseline[axis];
    values[6] = base_n > 0 && base_n == n ? (float)n : 0;
    free(figures);
'''
    driver.write_text(f'''#include "{header.name}"
{extra_headers}
#include <stdio.h>
#include <stdbool.h>
#include <stdlib.h>
#include <pthread.h>
void probe_input(int, float[3]);
static void *probe(void *unused) {{
  (void)unused;
  for (int mode = 0; mode < 5; ++mode) {{
    float values[11] = {{0}};
    float feet[3];
    probe_input(mode, feet);
    drawn_feet_probe(feet[0], feet[1], feet[2], (coax_v_f32){{values, 11}});
{figure_check}
    printf("%d", mode);
    for (int i = 0; i < 11; ++i) printf(" %.9g", (double)values[i]);
    putchar('\\n');
  }}
  return NULL;
}}
int main(void) {{
  pthread_attr_t attr;
  pthread_t thread;
  if (pthread_attr_init(&attr) || pthread_attr_setstacksize(&attr, 128 * 1024 * 1024)) return 2;
  if (pthread_create(&thread, &attr, probe, NULL)) return 3;
  pthread_attr_destroy(&attr);
  return pthread_join(thread, NULL) ? 4 : 0;
}}
''')
    # Unused host imports are not called by these pure frame probes. Keep the
    # same native-link allowance as state-defaults.coax; a called missing
    # effect still fails the executable rather than receiving a fake value.
    loose = "-Wl,-undefined,dynamic_lookup" if sys.platform == "darwin" else "-Wl,--unresolved-symbols=ignore-all"
    sanitizers = ["-g", "-fno-omit-frame-pointer", "-fsanitize=address,undefined", "-fno-sanitize-recover=all"] if sanitize else []
    run([cc, "-std=c11", "-O1", "-ffp-contract=off", "-Wno-unused-function", *sanitizers, str(code), str(driver), *libraries, "-o", str(binary), "-lm", "-pthread", loose])
    lines = run([str(binary)]).decode().splitlines()
    assert len(lines) == 5, (case, lines)
    for mode, line in enumerate(lines):
        values = [float(v) for v in line.split()]
        assert len(values) == 12 and values[0] == mode, (case, line)
        root, origin, emitted = values[1:4], values[4:7], values[7]
        assert emitted > 0, f"{case}: no actual figure/root record emitted: {line}"
        wanted = expected_root(case, origin)
        agrees = matches(root, wanted)
        assert agrees == (mode == 0), f"{case}: mode {mode}, rendered {root}, expected {wanted}"
    profile = "release" if release else "debug"
    if sanitize:
        profile += "+ASan/UBSan"
    print(f"{case} ({profile}): native rendered root agrees; dropped height, double height, last-step and spurious floor offset controls detected", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--coax", default=os.environ.get("COAX", str(ROOT / "coax/bin/coax")))
    parser.add_argument("--only", choices=CASES, nargs="+", default=CASES)
    parser.add_argument("--cc", default=os.environ.get("CC", "cc"))
    parser.add_argument("--release", action="store_true", help="generate every Coax unit in release mode")
    parser.add_argument("--sanitize", action="store_true", help="instrument all native units with AddressSanitizer and UndefinedBehaviorSanitizer")
    args = parser.parse_args()
    compiler = str(Path(args.coax).resolve())
    with tempfile.TemporaryDirectory(prefix="aethyr-drawn-feet-") as work:
        for case in args.only:
            check(case, compiler, Path(work), args.cc, args.release, args.sanitize)


if __name__ == "__main__":
    main()
