#!/usr/bin/env python3
"""Logical modules through the real driver (spec 17.1, r1a-logical-declarations).

ModuleIdentity: `#module` owners, #load membership and the nominal
declaration IDs that `coax deps --declarations` reports from the checked graph.
QualifiedLookup: `#load "FILE.coax" as NAME;` path imports and NAME.Decl
bindings resolved in the checked graph (r1a-qualified-lookup).
Visibility: `#export` lists seal a module; private declarations are refused
outside it, and the real verse/hud.coax graph keeps its native API
(r1a-public-hud-module).
TargetSpans: the native-lp64 target record against an independent C oracle,
unsupported layout targets, and exact authored declaration spans
(r1a-target-spans).
The tuple encoding and membership rules are checked and mutation-tested in
lib/compiler/sources.coax; these fixtures pin the composed behavior.
"""
from pathlib import Path
import json
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()
PREFIX = "coax.decl.v1:"

HUD = {
    "hud.coax": '#module "engine.hud";\n#load "hud/types.coax";\n#load "hud/frame.coax";\nHUD_BASE :: cast(int) 2;\n',
    "hud/types.coax": "HudSlot :: struct { index: int; }\nHudMode :: enum u8 { idle; busy; }\n",
    "hud/frame.coax": '#load "types.coax";\nhud_frame :: (s: HudSlot) -> int { return s.index + HUD_BASE; }\n',
    "main.coax": '#import "Basic";\n#load "hud.coax";\nmain :: () { print("%\\n", hud_frame(HudSlot.{index=40})); }\n',
}


class ModuleIdentity(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-modules-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name).resolve()

    def tree(self, files, under="a"):
        root = self.base / under
        for name, text in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        return root

    def run_coax(self, *args):
        return subprocess.run([str(COAX), *map(str, args)], capture_output=True, text=True)

    def report(self, main):
        p = self.run_coax("deps", main, "--declarations")
        self.assertEqual(p.returncode, 0, p.stderr)
        r = json.loads(p.stdout)
        self.assertEqual(r["schema"], "coax.declarations.v1")
        return r

    def ids(self, main):
        return {(d["kind"], d["name"]): d["id"] for d in self.report(main)["declarations"]}

    def error(self, main, code, file=None):
        p = self.run_coax("check", main, "--json")
        self.assertNotEqual(p.returncode, 0, p.stdout)
        records = [json.loads(line) for line in p.stderr.splitlines()]
        hits = [r for r in records if r["code"] == code]
        self.assertTrue(hits, records)
        if file:
            self.assertTrue(any(Path(r["file"]) == file for r in hits), records)
        return hits[0]

    def test_owner_and_members_share_one_logical_module(self):
        root = self.tree(HUD)
        self.assertEqual(self.run_coax("run", root / "main.coax").stdout, "42\n")
        r = self.report(root / "main.coax")
        units = {Path(u["path"]).relative_to(root).as_posix(): u for u in r["units"]}
        self.assertEqual(units["main.coax"]["module"], None)
        self.assertEqual(units["hud.coax"], {**units["hud.coax"], "module": "engine.hud", "claims": True})
        for member in ["hud/types.coax", "hud/frame.coax"]:
            self.assertEqual((units[member]["module"], units[member]["claims"]), ("engine.hud", False))
        ids = self.ids(root / "main.coax")
        self.assertEqual(ids[("struct", "HudSlot")], PREFIX + "engine.hud:struct:HudSlot")
        self.assertEqual(ids[("enum", "HudMode")], PREFIX + "engine.hud:enum:HudMode")
        self.assertEqual(ids[("const", "HUD_BASE")], PREFIX + "engine.hud:const:HUD_BASE")
        self.assertEqual(ids[("proc", "hud_frame")], PREFIX + "engine.hud:proc:hud_frame")
        # Unannotated legacy identities are explicitly unavailable.
        self.assertIsNone(ids[("proc", "main")])
        # Resolved references carry the target's nominal ID. (The checker
        # inlines untyped literal constants, so only typed ones remain references.)
        refs = {(x["from"], x["to"]) for x in r["references"]}
        self.assertIn((None, PREFIX + "engine.hud:proc:hud_frame"), refs)
        self.assertIn((PREFIX + "engine.hud:proc:hud_frame", PREFIX + "engine.hud:struct:HudSlot"), refs)
        self.assertIn((PREFIX + "engine.hud:proc:hud_frame", PREFIX + "engine.hud:const:HUD_BASE"), refs)
        self.assertEqual(r, self.report(root / "main.coax"))

    def test_relocation_and_reordering_preserve_nominal_ids(self):
        a = self.tree(HUD, "a")
        expected = self.ids(a / "main.coax")
        moved = self.tree(HUD, "elsewhere/deeper/b")
        self.assertEqual(self.ids(moved / "main.coax"), expected)
        reordered = dict(HUD)
        reordered["hud.coax"] = 'HUD_BASE :: 2;\n#load "hud/frame.coax";\n#module "engine.hud";\n#load "hud/types.coax";\n'
        reordered["hud/types.coax"] = "HudMode :: enum u8 { idle; busy; }\n\n\nHudSlot :: struct { index: int; }\n"
        c = self.tree(reordered, "c")
        self.assertEqual(self.ids(c / "main.coax"), expected)
        self.assertEqual(self.run_coax("run", c / "main.coax").stdout, "42\n")

    def test_moving_a_declaration_between_members_keeps_its_id(self):
        expected = self.ids(self.tree(HUD, "a") / "main.coax")
        moved = dict(HUD)
        moved["hud.coax"] = '#module "engine.hud";\n#load "hud/types.coax";\n#load "hud/frame.coax";\n'
        moved["hud/types.coax"] = HUD["hud/types.coax"] + "HUD_BASE :: cast(int) 2;\n"
        b = self.tree(moved, "b")
        self.assertEqual(self.ids(b / "main.coax"), expected)
        unit = next(d["unit"] for d in self.report(b / "main.coax")["declarations"] if d["name"] == "HUD_BASE")
        self.assertEqual(Path(unit), b / "hud/types.coax")

    def test_kind_name_and_module_changes_change_ids(self):
        base = self.ids(self.tree(HUD, "a") / "main.coax")
        renamed = dict(HUD)
        renamed["hud.coax"] = HUD["hud.coax"].replace("engine.hud", "engine.overlay")
        ids = self.ids(self.tree(renamed, "b") / "main.coax")
        self.assertEqual(ids[("struct", "HudSlot")], PREFIX + "engine.overlay:struct:HudSlot")
        self.assertTrue(all(ids[k] != v for k, v in base.items() if v))
        kind = dict(HUD)
        kind["hud/types.coax"] = "HudSlot :: struct { index: int; }\nHudMode :: struct { busy: bool; }\n"
        ids = self.ids(self.tree(kind, "c") / "main.coax")
        self.assertEqual(ids[("struct", "HudMode")], PREFIX + "engine.hud:struct:HudMode")
        self.assertNotIn(("enum", "HudMode"), ids)
        name = dict(HUD)
        name["hud/types.coax"] = HUD["hud/types.coax"].replace("HudMode", "HudState")
        ids = self.ids(self.tree(name, "d") / "main.coax")
        self.assertEqual(ids[("enum", "HudState")], PREFIX + "engine.hud:enum:HudState")
        self.assertEqual(ids[("struct", "HudSlot")], base[("struct", "HudSlot")])

    def test_independent_same_spelling_declarations_are_distinct(self):
        # Same name, same layout, different logical owners: different identities.
        left = self.tree({"vec.coax": '#module "math.left";\nVec :: struct { x: f32; y: f32; }\n',
                          "main.coax": '#load "vec.coax";\nmain :: () { v := Vec.{x=1, y=2}; #assert v.x < v.y; }\n'}, "left")
        right = self.tree({"vec.coax": '#module "math.right";\nVec :: struct { x: f32; y: f32; }\n',
                           "main.coax": '#load "vec.coax";\nmain :: () { v := Vec.{x=1, y=2}; #assert v.x < v.y; }\n'}, "right")
        a, b = self.ids(left / "main.coax")[("struct", "Vec")], self.ids(right / "main.coax")[("struct", "Vec")]
        self.assertNotEqual(a, b)
        self.assertEqual((a, b), (PREFIX + "math.left:struct:Vec", PREFIX + "math.right:struct:Vec"))
        # One graph may hold both: each module scopes its own declarations, and
        # an unqualified use that two modules could mean is ambiguous (QualifiedLookup).
        both = self.tree({"l.coax": '#module "math.left";\nVec :: struct { x: f32; }\n',
                          "r.coax": '#module "math.right";\nVec :: struct { x: f32; }\n',
                          "main.coax": '#load "l.coax";\n#load "r.coax";\nmain :: () {}\n'}, "both")
        p = self.run_coax("check", both / "main.coax")
        self.assertEqual(p.returncode, 0, p.stderr)
        (both / "main.coax").write_text('#load "l.coax";\n#load "r.coax";\nmain :: () { v: Vec; }\n')
        self.error(both / "main.coax", "E0386")

    def test_two_units_cannot_claim_one_module(self):
        root = self.tree({"a.coax": '#module "engine.hud";\nA :: 1;\n',
                          "b.coax": '#module "engine.hud";\nB :: 2;\n',
                          "main.coax": '#load "a.coax";\n#load "b.coax";\nmain :: () {}\n'})
        record = self.error(root / "main.coax", "E0606", root / "b.coax")
        self.assertEqual(record["line"], 1)
        self.assertIn("a.coax", record["message"])

    def test_unclaimed_member_under_two_owners_fails(self):
        root = self.tree({"shared.coax": "Shared :: struct { v: int; }\n",
                          "a.coax": '#module "engine.a";\n#load "shared.coax";\n',
                          "b.coax": '#module "engine.b";\n\n#load "shared.coax";\n',
                          "main.coax": '#load "a.coax";\n#load "b.coax";\nmain :: () {}\n'})
        record = self.error(root / "main.coax", "E0607", root / "b.coax")
        self.assertEqual(record["line"], 3)
        for name in ["shared.coax", "engine.a", "engine.b"]:
            self.assertIn(name, record["message"])
        # Claiming the shared unit makes it its own owner; both modules may load it.
        (root / "shared.coax").write_text('#module "engine.shared";\nShared :: struct { v: int; }\n')
        self.assertEqual(self.ids(root / "main.coax")[("struct", "Shared")], PREFIX + "engine.shared:struct:Shared")

    def test_diamonds_and_unowned_loaders_keep_one_owner(self):
        # hud/types.coax is reached twice inside engine.hud (a canonical diamond)
        # and once more straight from the unannotated main, which owns nothing.
        files = dict(HUD)
        files["main.coax"] = '#import "Basic";\n#load "hud/types.coax";\n#load "hud.coax";\nmain :: () { print("%\\n", hud_frame(HudSlot.{index=40})); }\n'
        root = self.tree(files)
        self.assertEqual(self.run_coax("run", root / "main.coax").stdout, "42\n")
        ids = self.ids(root / "main.coax")
        self.assertEqual(ids[("struct", "HudSlot")], PREFIX + "engine.hud:struct:HudSlot")
        self.assertEqual(len(self.report(root / "main.coax")["units"]), 4)

    def test_invalid_module_directives(self):
        root = self.tree({"main.coax": "main :: () {}\n"})
        main = root / "main.coax"
        for directive in ['#module "";', '#module "Engine";', '#module "a..b";', '#module "a.";', '#module 4;',
                          '#module "a"', '#module', '#module "a";\n#module "b";']:
            main.write_text(directive + "\nmain :: () {}\n")
            self.error(main, "E0605", main)
        main.write_text('main :: () {#module "a";}\n')
        self.error(main, "E0605", main)
        # Comments and strings never become claims.
        main.write_text('// #module "a";\nmain :: () { s := "#module \\"b\\";"; #assert s.count > 0; }\n')
        r = self.report(main)
        self.assertEqual(r["units"][0]["module"], None)

    def test_member_diagnostics_and_authority_stay_with_their_units(self):
        files = dict(HUD)
        files["hud/frame.coax"] = '#load "types.coax";\n// line two\nhud_frame :: (s: HudSlot) -> int { return s.index + missing; }\n'
        root = self.tree(files)
        record = self.error(root / "main.coax", "E0306", root / "hud/frame.coax")
        self.assertEqual(record["line"], 3)
        files["hud/frame.coax"] = '#authority;\n#load "types.coax";\nhud_frame :: (s: HudSlot) -> int { return s.index + HUD_BASE; }\n#test "frame" {#assert hud_frame(HudSlot.{index=1})==4;}\n'
        root = self.tree(files, "b")
        self.error(root / "main.coax", "E0406", root / "hud/frame.coax")
        (root / "hud/frame.coax").write_text(files["hud/frame.coax"].replace("==4", "==3"))
        self.assertEqual(self.run_coax("run", root / "main.coax").stdout, "42\n")
        ids = self.ids(root / "main.coax")
        self.assertEqual(ids[("proc", "hud_frame")], PREFIX + "engine.hud:proc:hud_frame")

    def test_a_claim_changes_no_generated_code_or_lines(self):
        root = self.tree({"plain.coax": "Pair :: struct { a: int; b: int; }\nsum :: (p: Pair) -> int #export_c { return p.a + p.b; }\n"})
        plain = self.run_coax("c", root / "plain.coax", "--library")
        self.assertEqual(plain.returncode, 0, plain.stderr)
        claimed = root / "claimed.coax"
        # Appended, so every authored line keeps its number.
        claimed.write_text((root / "plain.coax").read_text() + '#module "math.pair";\n')
        out = self.run_coax("c", claimed, "--library")
        self.assertEqual(out.returncode, 0, out.stderr)
        strip = lambda text: [l for l in text.splitlines() if "plain.coax" not in l and "claimed.coax" not in l]
        self.assertEqual(strip(out.stdout), strip(plain.stdout))
        # The schema-1 path inventory is unchanged by the opt-in report.
        deps = json.loads(self.run_coax("deps", claimed).stdout)
        self.assertEqual(deps["schema_version"], 1)
        self.assertNotIn("declarations", deps)
        line = next(d["line"] for d in self.report(claimed)["declarations"] if d["name"] == "sum")
        self.assertEqual(line, 2)


GUARD = {
    "types.coax": '#module "game.guard_types";\nGaObservation :: struct { calm: bool; day: int; }\nGaEffect :: struct { flags: int; }\n',
    "util.coax": "ga_clamp :: (x: int) -> int { return ifx x < 0 then 0 else x; }\n",
    "policy.coax": '#module "game.guard_policy";\n#load "types.coax";\n#load "util.coax";\nga_decide :: (o: GaObservation) -> GaEffect #export_c { return GaEffect.{flags=ga_clamp(o.day)}; }\n',
    "page.coax": '#load "types.coax";\npage_frame :: (o: GaObservation) -> int #export_c { return o.day; }\n',
}


class ModuleRevisions(unittest.TestCase):
    """Conservative module-scoped revisions, kept apart from nominal IDs."""
    setUp = ModuleIdentity.setUp
    tree = ModuleIdentity.tree
    run_coax = ModuleIdentity.run_coax

    def modules(self, main):
        p = self.run_coax("deps", main, "--modules", "--library")
        self.assertEqual(p.returncode, 0, p.stderr)
        r = json.loads(p.stdout)
        self.assertEqual(r["schema"], "coax.modules.v1")
        return r

    def revs(self, main):
        r = self.modules(main)
        return ({m["id"]: (m["content_revision"], m["revision"]) for m in r["modules"]},
                {d["id"]: d["revision"] for d in r["declarations"]})

    def test_two_consumers_share_ids_and_revisions(self):
        root = self.tree(GUARD)
        page, policy = self.revs(root / "page.coax"), self.revs(root / "policy.coax")
        for name in ["GaObservation", "GaEffect"]:
            key = PREFIX + "game.guard_types:struct:" + name
            self.assertEqual(page[1][key], policy[1][key])
        self.assertEqual(page[0]["game.guard_types"], policy[0]["game.guard_types"])
        r = self.modules(root / "policy.coax")
        self.assertTrue(all(len(m["revision"]) == 64 for m in r["modules"]))
        self.assertIn("execution_profile", r["profile"])

    def test_relocation_and_traversal_keep_revisions(self):
        a = self.revs(self.tree(GUARD, "a") / "policy.coax")
        self.assertEqual(self.revs(self.tree(GUARD, "x/y/b") / "policy.coax"), a)
        moved = dict(GUARD)
        moved["policy.coax"] = '#load "util.coax";\nga_decide :: (o: GaObservation) -> GaEffect #export_c { return GaEffect.{flags=ga_clamp(o.day)}; }\n#load "types.coax";\n#module "game.guard_policy";\n'
        c = self.revs(self.tree(moved, "c") / "policy.coax")
        self.assertEqual(c[0]["game.guard_types"], a[0]["game.guard_types"])
        self.assertEqual(c[1][PREFIX + "game.guard_types:struct:GaEffect"], a[1][PREFIX + "game.guard_types:struct:GaEffect"])
        # The policy's own source changed (reordered lines), so its revision changes, conservatively.
        self.assertNotEqual(c[0]["game.guard_policy"], a[0]["game.guard_policy"])
        unrelated = dict(GUARD)
        unrelated["extra.coax"] = "unrelated :: () -> int { return 7; }\n"
        unrelated["page.coax"] = '#load "extra.coax";\n' + GUARD["page.coax"]
        d = self.revs(self.tree(unrelated, "d") / "page.coax")
        self.assertEqual(d[0]["game.guard_types"], a[0]["game.guard_types"])

    def test_content_layout_and_dependency_edits(self):
        base = self.revs(self.tree(GUARD, "a") / "policy.coax")
        key = PREFIX + "game.guard_types:struct:GaObservation"
        layout = dict(GUARD)
        layout["types.coax"] = GUARD["types.coax"].replace("day: int;", "day: int; weather: int;")
        r = self.revs(self.tree(layout, "b") / "policy.coax")
        self.assertIn(key, r[1])  # the nominal ID is unchanged
        self.assertNotEqual(r[1][key], base[1][key])
        self.assertNotEqual(r[0]["game.guard_types"][0], base[0]["game.guard_types"][0])
        # A dependency's change reaches its dependents' revisions, not their content.
        self.assertEqual(r[0]["game.guard_policy"][0], base[0]["game.guard_policy"][0])
        self.assertNotEqual(r[0]["game.guard_policy"][1], base[0]["game.guard_policy"][1])
        body = dict(GUARD)
        body["util.coax"] = GUARD["util.coax"].replace("then 0", "then 1")
        r = self.revs(self.tree(body, "c") / "policy.coax")
        # util.coax has no claim, so it is a member of the policy: a body edit
        # changes the policy's content and revision, never the types module.
        self.assertEqual(r[0]["game.guard_types"], base[0]["game.guard_types"])
        self.assertNotEqual(r[0]["game.guard_policy"][0], base[0]["game.guard_policy"][0])
        self.assertNotEqual(r[0]["game.guard_policy"][1], base[0]["game.guard_policy"][1])
        comment = dict(GUARD)
        comment["policy.coax"] = "// a comment is content too\n" + GUARD["policy.coax"]
        r = self.revs(self.tree(comment, "d") / "policy.coax")
        self.assertNotEqual(r[0]["game.guard_policy"][0], base[0]["game.guard_policy"][0])

    def test_unavailable_identities_are_named_not_invented(self):
        root = self.tree(GUARD)
        r = self.modules(root / "page.coax")
        un = r["unavailable"]
        self.assertNotIn("declaration_ranges", un)
        self.assertIn("declaration_revisions", un)
        self.assertEqual([Path(u["path"]).name for u in un["legacy_units"]], ["page.coax"])
        self.assertEqual([d["name"] for d in un["legacy_declarations"]], ["page_frame"])
        self.assertNotIn("page_frame", json.dumps(r["declarations"]))
        (root / "proto").mkdir()
        shutil.copy(ROOT.parent / "proto/descriptors/ios-home.pb", root / "proto/types.pb")
        (root / "proto/codec.coax").write_text('#module "game.wire";\n#import_proto "types.pb" as Wire;\nwire_rev :: () -> u64 { return Wire_HudInvocation_get_revision(Wire_HudInvocation_new()).value; }\n')
        (root / "wire.coax").write_text('#load "proto/codec.coax";\nmain :: () {}\n')
        r = self.modules(root / "wire.coax")
        wire = next(m for m in r["modules"] if m["id"] == "game.wire")
        self.assertTrue(any("descriptor" in d for d in wire["dependencies"]))
        self.assertTrue(r["unavailable"]["generated_declarations"])
        self.assertTrue(all(d["generated"] for d in r["declarations"] if "Wire_" in d["id"]))


# Two modules that spell the same local names differently.
PAIR = {
    "left.coax": '#module "demo.left";\nV :: struct { x: int; }\nE :: enum u8 { lo; hi; }\nK :: cast(int) 10;\ng :: () -> int { return 1; }\nscale :: (v: V) -> int { return v.x * K; }\n',
    "right.coax": '#module "demo.right";\nV :: struct { x: int; }\nK :: cast(int) 20;\ng :: () -> int { return 2; }\n',
    "main.coax": '#import "Basic";\n#load "left.coax" as l;\n#load "right.coax" as r;\n'
                 'main :: () {\n  a: l.V = l.V.{x = l.g()};\n  b := r.V.{x = r.g()};\n  e := l.E.hi;\n'
                 '  print("% % % % % %\\n", a.x, b.x, l.K, r.K, l.scale(a), cast(int) e);\n}\n',
}
PAIR_OUT = "1 2 10 20 10 1\n"


class QualifiedLookup(unittest.TestCase):
    """Qualified names bind through the checked graph to originating declarations."""
    setUp = ModuleIdentity.setUp
    tree = ModuleIdentity.tree
    run_coax = ModuleIdentity.run_coax
    report = ModuleIdentity.report
    ids = ModuleIdentity.ids
    error = ModuleIdentity.error

    def ok(self, *args):
        p = self.run_coax(*args)
        self.assertEqual(p.returncode, 0, p.stderr)
        return p.stdout

    def at(self, main, code, line, column, text=None):
        d = self.error(main, code)
        self.assertEqual((Path(d["file"]).name, d["line"], d["col"]), (main.name, line, column), d)
        if text:
            self.assertIn(text, d["message"])
        return d

    def case(self, body, under, extra=None):
        files = dict(PAIR)
        files["main.coax"] = '#import "Basic";\n#load "left.coax" as l;\n#load "right.coax" as r;\n' + body
        files.update(extra or {})
        return self.tree(files, under) / "main.coax"

    def test_colliding_names_resolve_in_every_backend(self):
        main = self.tree(PAIR) / "main.coax"
        self.assertEqual(self.ok("run", main), PAIR_OUT)
        exe = self.base / "pair"
        self.ok("build", main, "-o", exe)
        self.assertEqual(subprocess.run([str(exe)], capture_output=True, text=True).stdout, PAIR_OUT)
        c = self.ok("c", main)
        # Colliding declarations get distinct C spellings; unique ones keep theirs.
        for spelling in ["coax_q1t_V", "coax_q2t_V", "coax_q1p_g", "coax_q2p_g", "coax_p_scale", "coax_t_E"]:
            self.assertIn(spelling, c)
        # The evaluator (compile-time) binds the same declarations.
        folded = self.case('X :: #run (l.g() * 100 + r.g() * 10 + l.K);\nmain :: () { print("%\\n", X); }\n', "ct")
        self.assertEqual(self.ok("run", folded), "130\n")

    def test_metal_scalar_kernel_binds_qualified_calls(self):
        files = {
            "warm.coax": '#module "shade.warm";\ngain :: (x: f32) -> f32 { return x * 0.5; }\n',
            "cool.coax": '#module "shade.cool";\ngain :: (x: f32) -> f32 { return x * 0.25; }\n',
            "shader.coax": '#load "warm.coax" as warm;\n#load "cool.coax" as cool;\n'
                           'shade :: (x: f32) -> f32 #export_c { return warm.gain(x) + cool.gain(x); }\n',
        }
        root = self.tree(files)
        metal = self.ok("metal", root / "shader.coax")
        self.assertIn("coax_q1p_gain(", metal)
        self.assertIn("coax_q2p_gain(", metal)
        self.assertIn("0.5", metal.split("coax_q1p_gain(", 2)[-1].split("}")[0] + metal)
        # The same file's C library computes the same values.
        lib = self.ok("c", root / "shader.coax", "--library")
        self.assertIn("coax_q1p_gain", lib)

    def test_unknown_ambiguous_and_missing_names_fail_at_the_use(self):
        self.at(self.case('main :: () { v: zz.V; }\n', "a"), "E0384", 4, 17, "'zz'")
        self.at(self.case('main :: () { v: V; }\n', "b"), "E0386", 4, 17, "'demo.left' and 'demo.right'")
        self.at(self.case('main :: () { x := g(); }\n', "c"), "E0386", 4, 19)
        self.at(self.case('main :: () { x := r.E.hi; }\n', "d"), "E0385", 4, 19, "module 'demo.right' has no declaration 'E'")
        self.at(self.case('main :: () { v: l.W; }\n', "e"), "E0385", 4, 17, "no type 'W'")
        self.at(self.case('main :: () { x := r.scale(r.V.{x=1}); }\n', "f"), "E0385", 4, 19, "no procedure 'scale'")
        self.at(self.case('main :: () { x := l.g; }\n', "g"), "E0305", 4, 19)
        self.at(self.case('main :: () { l.K = 3; }\n', "h"), "E0344", 4, 18)
        # A qualified name never falls back to a builtin type.
        self.at(self.case('main :: () { v: l.int; }\n', "i"), "E0385", 4, 17)

    def test_aliases_do_not_change_ids_and_same_layout_stays_nominal(self):
        base = self.ids(self.tree(PAIR, "a") / "main.coax")
        renamed = dict(PAIR)
        renamed["main.coax"] = PAIR["main.coax"].replace(" as l;", " as left;").replace("l.", "left.")
        moved = self.tree(renamed, "b") / "main.coax"
        self.assertEqual(self.ok("run", moved), PAIR_OUT)
        self.assertEqual(self.ids(moved), base)
        report = self.report(self.tree(PAIR, "c") / "main.coax")
        ids = {d["id"] for d in report["declarations"]}
        self.assertIn(PREFIX + "demo.left:struct:V", ids)
        self.assertIn(PREFIX + "demo.right:struct:V", ids)
        d = self.at(self.case('main :: () { v: l.V = r.V.{x=1}; }\n', "d"), "E0325", 4, 23)
        self.assertIn("demo.left.V", d["message"])
        self.assertIn("demo.right.V", d["message"])
        self.at(self.case('f :: (v: r.V) {}\nmain :: () { f(l.V.{x=1}); }\n', "e"), "E0317", 5, 16)

    def test_unsupported_forms_fail_explicitly(self):
        self.at(self.case('main :: () { v: l.r.V; }\n', "a"), "E0222", 4, 17)
        self.at(self.case('main :: () { x := l.r.g(); }\n', "b"), "E0222", 4, 19)
        self.at(self.case('main :: () { v := l.r.V.{x=1}; }\n', "c"), "E0222", 4, 19)
        inner = {"left.coax": PAIR["left.coax"] + '#load "right.coax" as r;\n'}
        self.at(self.case('main :: () { x := l.r.K; }\n', "d", inner), "E0388", 4, 19, "'l.r'")
        for text, code in [('#load "left.coax" as l(n=1);\nmain :: () {}\n', "E0608"),
                           ('#load "left" as l;\nmain :: () {}\n', "E0609"),
                           ('#load "left.coax" as 7;\nmain :: () {}\n', "E0600")]:
            root = self.tree(dict(PAIR, **{"m.coax": text}), "x" + code)
            self.at(root / "m.coax", code, 1, 1)
        plain = self.tree(dict(PAIR, **{"plain.coax": "p :: () {}\n", "m.coax": '#load "plain.coax" as p;\nmain :: () {}\n'}), "y")
        self.at(plain / "m.coax", "E0610", 1, 23, "plain.coax'")
        twice = self.tree(dict(PAIR, **{"m.coax": '#load "left.coax" as a;\n#load "right.coax" as a;\nmain :: () {}\n'}), "z")
        self.at(twice / "m.coax", "E0611", 2, 23, "'demo.left'")
        same = self.tree(dict(PAIR, **{"m.coax": '#load "left.coax" as a;\n#load "left.coax" as a;\nmain :: () {}\n'}), "w")
        self.ok("check", same / "m.coax")

    def test_c_symbol_collisions_are_refused_not_merged(self):
        files = {
            "one.coax": '#module "lib.one";\nopen_it :: () -> int #export_c { return 1; }\nT :: struct { a: int; }\n',
            "two.coax": '#module "lib.two";\nopen_it :: () -> int #export_c { return 2; }\n',
            "main.coax": '#load "one.coax" as one;\n#load "two.coax" as two;\n',
        }
        root = self.tree(files)
        d = self.error(root / "main.coax", "E0387")
        self.assertIn("C symbol 'open_it'", d["message"])
        types = {
            "one.coax": '#module "lib.one";\nT :: struct { a: int; }\nget_one :: (t: T) -> int #export_c { return t.a; }\n',
            "two.coax": '#module "lib.two";\nT :: struct { a: int; }\n',
            "main.coax": '#load "one.coax" as one;\n#load "two.coax" as two;\n',
        }
        root = self.tree(types, "b")
        p = self.run_coax("c", root / "main.coax", "--library")
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("E0387", p.stderr)
        self.assertIn("C library header spells types by name", p.stderr)

    def test_legacy_load_graphs_are_unchanged(self):
        root = self.tree(HUD)
        self.assertEqual(self.ok("run", root / "main.coax"), "42\n")
        flat = dict(HUD)
        flat["hud.coax"] = HUD["hud.coax"].replace('#module "engine.hud";\n', "")
        c_flat = self.ok("c", self.tree(flat, "f") / "main.coax")
        self.assertNotIn("coax_q", c_flat)
        # Same-scope duplicates are still duplicates.
        dup = self.tree({"main.coax": 'g :: () {}\ng :: () {}\nmain :: () {}\n'}, "d")
        self.error(dup / "main.coax", "E0334")
        dup = self.tree({"x.coax": '#module "m.x";\nV :: struct {}\nV :: struct {}\n', "main.coax": '#load "x.coax" as x;\nmain :: () {}\n'}, "e")
        self.error(dup / "main.coax", "E0372")
# A sealed module (an #export list) whose member unit uses its private names,
# loaded by a second module that is not sealed.
SEALED = {
    "geo/geo.coax": '#module "demo.geo";\n#export Rect, area, UNIT;\n#load "inner.coax";\nRect :: struct { w: int; h: int; }\nUNIT :: cast(int) 1;\narea :: (r: Rect) -> int { return scale(r.w * r.h); }\n',
    "geo/inner.coax": '#authority;\nSCALE :: cast(int) 2;\nPad :: struct { p: int; }\nscale :: (v: int) -> int { p := Pad.{p = SCALE}; return v * p.p; }\n#test "members use private names" { #assert scale(3) == 6; }\n',
    "mid.coax": '#module "demo.mid";\n#load "geo/geo.coax" as geo;\nmid_area :: () -> int { return geo.area(geo.Rect.{w = 2, h = 3}); }\nMID :: cast(int) 5;\n',
    "main.coax": '#import "Basic";\n#load "mid.coax" as mid;\n#load "geo/geo.coax" as geo;\n'
                 'main :: () { r := geo.Rect.{w = 4, h = 5}; print("% % % %\\n", geo.area(r), mid.mid_area(), mid.MID, geo.UNIT); }\n',
}
HUD_EXPORTS = ["HudPoint", "HudRect", "verse_hud_arm_down", "verse_hud_cut", "verse_hud_frame_arm", "verse_hud_frame_point",
               "verse_hud_outline", "verse_hud_slot", "verse_hud_tone"]


class Visibility(unittest.TestCase):
    """Private-by-default declarations in modules with an #export list."""
    setUp = ModuleIdentity.setUp
    tree = ModuleIdentity.tree
    run_coax = ModuleIdentity.run_coax
    error = ModuleIdentity.error
    ok = QualifiedLookup.ok

    def refused(self, body, code, under):
        files = dict(SEALED)
        files["main.coax"] = '#import "Basic";\n#load "mid.coax" as mid;\n#load "geo/geo.coax" as geo;\nmain :: () { ' + body + ' }\n'
        return self.error(self.tree(files, under) / "main.coax", code)

    def test_exported_names_stay_usable_and_members_share_the_owner(self):
        root = self.tree(SEALED)
        self.assertEqual(self.ok("run", root / "main.coax"), "40 12 5 1\n")
        # The member unit's #authority and its check stay with that physical unit.
        out = self.ok("test", root / "geo/geo.coax", "--lib")
        self.assertIn("members use private names", out)
        self.assertIn("1 check, 1 passed, 0 failed", out)
        m = json.loads(self.ok("deps", root / "main.coax", "--modules"))
        exports = {x["id"]: x["exports"] for x in m["modules"]}
        self.assertEqual(exports, {"demo.geo": ["Rect", "UNIT", "area"], "demo.mid": None})

    def test_private_declarations_are_refused_outside_their_module(self):
        cases = [("x := geo.SCALE;", "'SCALE' is private to module 'demo.geo'"),
                 ("x := geo.scale(1);", "'scale' is private to module 'demo.geo'"),
                 ("p := geo.Pad.{p = 1};", "'Pad' is private to module 'demo.geo'"),
                 ("x := SCALE;", "'SCALE' is private to module 'demo.geo'"),
                 ("x := scale(2);", "'scale' is private to module 'demo.geo'"),
                 ("p: Pad;", "'Pad' is private to module 'demo.geo'")]
        for i, (body, message) in enumerate(cases):
            hit = self.refused(body, "E0389", f"p{i}")
            self.assertEqual(hit["message"], message)
            self.assertEqual(Path(hit["file"]).name, "main.coax")
        # Transitive access: demo.mid's aliases and loaded names are not reexported.
        self.refused("x := mid.geo.UNIT;", "E0388", "t0")
        self.refused("x := mid.area(mid.Rect.{w = 1, h = 1});", "E0385", "t1")
        self.refused("x := mid.SCALE;", "E0385", "t2")

    def test_export_lists_name_own_declarations_once(self):
        def geo(text, under):
            files = dict(SEALED)
            files["geo/geo.coax"] = SEALED["geo/geo.coax"].replace("#export Rect, area, UNIT;", text)
            return self.tree(files, under) / "main.coax"
        hit = self.error(geo("#export Rect, area, UNIT, MID;", "u"), "E0390")
        self.assertEqual(hit["message"], "module 'demo.geo' exports 'MID', which it does not declare")
        self.assertEqual((Path(hit["file"]).name, hit["line"]), ("geo.coax", 2))
        self.error(geo("#export Rect, area, UNIT;\n#export area;", "d"), "E0391")
        self.error(geo("#export Rect, mid.MID;", "r"), "E0614")
        for i, text in enumerate(("#export Rect,;", "#export;", "#export Rect area;", '#export "Rect";')):
            self.error(geo(text, f"s{i}"), "E0613")
        lone = self.tree({"main.coax": "#export x;\nx :: 1;\nmain :: () {}\n"}, "l")
        hit = self.error(lone / "main.coax", "E0615")
        self.assertEqual((hit["line"], hit["col"]), (1, 9))

    def test_unannotated_graphs_keep_every_name_visible(self):
        files = dict(SEALED)
        files["geo/geo.coax"] = SEALED["geo/geo.coax"].replace("#export Rect, area, UNIT;\n", "")
        files["main.coax"] = '#import "Basic";\n#load "geo/geo.coax" as geo;\nmain :: () { print("% %\\n", geo.SCALE, scale(4)); }\n'
        self.assertEqual(self.ok("run", self.tree(files) / "main.coax"), "2 8\n")
        root = self.tree(HUD, "h")
        self.assertEqual(self.ok("run", root / "main.coax"), "42\n")

    def test_the_hud_graph_exposes_its_native_api(self):
        hud = ROOT.parent / "verse/hud.coax"
        self.assertIn("3 checks, 3 passed", self.ok("test", hud))
        m = json.loads(self.ok("deps", hud, "--modules"))
        (mod,) = m["modules"]
        self.assertEqual(mod["id"], "verse.hud")
        self.assertEqual(mod["exports"], HUD_EXPORTS)
        self.assertEqual(sorted(Path(x["path"]).name for x in mod["members"]), ["frame.coax", "hud.coax", "types.coax"])
        # A page in that graph cannot reach the private HUD_* constants.
        page = self.base / "page.coax"
        page.write_text('#load "%s" as hud;\nx :: () -> f32 { return hud.HUD_ARM; }\nmain :: () {}\n' % hud)
        self.assertEqual(self.error(page, "E0389")["message"], "'HUD_ARM' is private to module 'verse.hud'")
        cc = shutil.which("cc") or shutil.which("clang")
        if not cc:
            self.skipTest("no native C compiler for the HUD host")
        header, lib = self.base / "hud.h", self.base / "hud.c"
        lib.write_text(self.ok("c", hud, "--library", "--header", header))
        host = self.base / "host.c"
        host.write_text('#include <stdio.h>\n#include "hud.h"\nint main(void) {\n'
                        '  coax_t_HudRect r = verse_hud_slot(320, 700, 34, 0, 6);\n'
                        '  coax_t_HudPoint p = verse_hud_frame_point(0, 0, 100, 40, 7, 1);\n'
                        '  printf("%zu %zu %g %g %g %g %g %g %g %g\\n", sizeof(coax_t_HudRect), sizeof(coax_t_HudPoint), r.width, r.height, p.x, p.y,\n'
                        '         verse_hud_frame_arm(400, 1), verse_hud_tone(4), verse_hud_cut(1), verse_hud_outline() + verse_hud_arm_down());\n'
                        '  return 0;\n}\n')
        exe = self.base / "host"
        subprocess.run([cc, "-std=c11", "-I", str(self.base), str(host), str(lib), "-lm", "-o", str(exe)], check=True)
        out = subprocess.run([str(exe)], capture_output=True, text=True, check=True).stdout.split()
        self.assertEqual(out[:2], ["16", "8"])
        for got, want in zip([float(v) for v in out[2:]], [44.0, 44.0, 93.0, 0.0, 22.0, 0.216, 10.0, 0.9]):
            self.assertAlmostEqual(got, want, places=5)


# Scalars by Coax name and the C type the oracle measures independently.
ORACLE_TYPES = [("bool", "_Bool"), ("f32", "float"), ("f64", "double"), ("s8", "int8_t"), ("s16", "int16_t"),
                ("s32", "int32_t"), ("int", "int64_t"), ("u8", "uint8_t"), ("u16", "uint16_t"), ("u32", "uint32_t"),
                ("u64", "uint64_t")]
SPANS = {
    "shapes.coax": '#module "demo.shapes";\n\n// a comment before is not part of the item\nPoint :: struct {\n  x: int;\n#if BUILD_DEBUG { tag: int; }\n  y: int;\n}\nMode :: enum u8 { a; b; }  // trailing\n',
    "main.coax": '#load "shapes.coax";\nLIMIT :: cast(int) 3;\nsq :: (p: Point) -> int {\n  return p.x * p.y;\n}\nmain :: () {}\n',
}


class TargetSpans(unittest.TestCase):
    """Target layout facts and exact original declaration ranges."""
    setUp = ModuleIdentity.setUp
    tree = ModuleIdentity.tree
    run_coax = ModuleIdentity.run_coax
    report = ModuleIdentity.report

    def oracle(self):
        cc = shutil.which("cc") or shutil.which("clang")
        if not cc:
            self.skipTest("no native C compiler for the layout oracle")
        lines = ["#include <stdio.h>", "#include <stdint.h>", "#include <stddef.h>", "#include <stdalign.h>",
                 "struct view { const void *data; int64_t count; };", "int main(void) {"]
        rows = [("pointer", "void *"), ("count", "int64_t"), ("view", "struct view")] + ORACLE_TYPES
        for name, ctype in rows:
            lines.append(f'  printf("%s %zu %zu\\n", "{name}", sizeof({ctype}), alignof({ctype}));')
        lines.append("  return 0;\n}\n")
        src = self.base / "oracle.c"
        src.write_text("\n".join(lines))
        exe = self.base / "oracle"
        subprocess.run([cc, "-std=c11", str(src), "-o", str(exe)], check=True)
        out = subprocess.run([str(exe)], capture_output=True, text=True, check=True).stdout
        return {n: (int(a), int(b)) for n, a, b in (l.split() for l in out.splitlines())}

    def test_native_lp64_facts_agree_with_the_c_oracle(self):
        t = self.report(self.tree(SPANS) / "main.coax")["target"]
        seen = self.oracle()
        if seen["pointer"] != (8, 8):
            self.skipTest("the host is not LP64")
        self.assertEqual((t["name"], t["data_model"], t["calling_convention"]), ("native-lp64", "LP64", "c"))
        for key in ("pointer", "count", "view"):
            self.assertEqual((t[key]["size"], t[key]["align"]), seen[key], key)
        scalars = {s["type"]: (s["size"], s["align"]) for s in t["scalars"]}
        self.assertEqual(sorted(scalars), sorted(n for n, _ in ORACLE_TYPES))
        for name, _ in ORACLE_TYPES:
            self.assertEqual(scalars[name], seen[name], name)
        # The checker lays a declared struct out with the same facts.
        layout = self.run_coax("layout", self.tree(SPANS, "l") / "main.coax")
        self.assertEqual(layout.returncode, 0, layout.stderr)
        # The development profile selects the #if field: three counts.
        self.assertIn("Point  size %d  align %d" % (3 * seen["count"][0], seen["count"][1]), layout.stdout)
        rel = self.run_coax("layout", self.tree(SPANS, "k") / "main.coax", "--build-profile", "release")
        self.assertEqual(rel.returncode, 0, rel.stderr)
        self.assertIn("Point  size %d  align %d" % (2 * seen["count"][0], seen["count"][1]), rel.stdout)
        explicit = self.run_coax("deps", self.tree(SPANS, "e") / "main.coax", "--declarations", "--target", "native-lp64")
        self.assertEqual(json.loads(explicit.stdout)["target"], t)

    def test_unsupported_targets_refuse_instead_of_borrowing_lp64(self):
        main = self.tree(SPANS) / "main.coax"
        for target in ("wasm32", "metal", "spirv", "native-ilp32"):
            for args in (("layout", main), ("deps", main, "--declarations"), ("deps", main, "--modules")):
                p = self.run_coax(*args, "--target", target)
                self.assertEqual(p.returncode, 1, (target, args))
                self.assertEqual(p.stdout, "")
                self.assertIn("E0612", p.stderr)
                self.assertIn(f"'{target}'", p.stderr)
        p = self.run_coax("check", main, "--target", "native-lp64")
        self.assertNotEqual(p.returncode, 0, "--target is a layout-query option only")

    def spans(self, main):
        r = self.report(main)
        units = {u["path"]: Path(u["path"]) for u in r["units"]}
        out = {}
        for d in r["declarations"]:
            span = d["span"]
            self.assertEqual(d["origin"], "authored")
            text = Path(units[next(u for u in units if d["unit"] == u)]).read_bytes()
            out[d["name"]] = (d["id"], span, text[span["start"]:span["end"]].decode())
            self.assertEqual(span["start_line"], text[:span["start"]].count(b"\n") + 1)
            self.assertEqual(span["end_line"], text[:span["end"]].count(b"\n") + 1)
        return out

    def test_spans_select_the_original_authored_bytes(self):
        s = self.spans(self.tree(SPANS) / "main.coax")
        self.assertEqual(s["Point"][2], "Point :: struct {\n  x: int;\n#if BUILD_DEBUG { tag: int; }\n  y: int;\n}")
        self.assertEqual(s["Mode"][2], "Mode :: enum u8 { a; b; }")
        self.assertEqual(s["LIMIT"][2], "LIMIT :: cast(int) 3;")
        self.assertEqual(s["sq"][2], "sq :: (p: Point) -> int {\n  return p.x * p.y;\n}")
        self.assertEqual(s["main"][2], "main :: () {}")
        self.assertEqual((s["Point"][1]["start_line"], s["Point"][1]["end_line"]), (4, 8))
        # The release profile selects the other #if form; the range is still the authored one.
        rel = self.run_coax("deps", self.tree(SPANS, "r") / "main.coax", "--declarations", "--build-profile", "release")
        self.assertEqual(rel.returncode, 0, rel.stderr)
        point = next(d for d in json.loads(rel.stdout)["declarations"] if d["name"] == "Point")
        self.assertEqual(point["span"], s["Point"][1])

    def test_relocation_keeps_ids_and_moves_ranges_truthfully(self):
        base = self.spans(self.tree(SPANS) / "main.coax")
        moved = dict(SPANS)
        moved["shapes.coax"] = SPANS["shapes.coax"].replace("// a comment before is not part of the item\n", "// a much longer comment\n// on two lines\n\n")
        moved["shapes.coax"] = moved["shapes.coax"].replace("Mode :: enum u8 { a; b; }  // trailing\n", "") .replace("#module \"demo.shapes\";\n", "#module \"demo.shapes\";\nMode :: enum u8 { a; b; }\n")
        after = self.spans(self.tree(moved, "m") / "main.coax")
        for name in base:
            self.assertEqual(after[name][0], base[name][0], name)
            self.assertEqual(after[name][2], base[name][2], name)
            self.assertEqual(after[name][1]["sha256"], base[name][1]["sha256"], name)
        self.assertNotEqual(after["Point"][1]["start"], base["Point"][1]["start"])
        self.assertLess(after["Mode"][1]["start"], after["Point"][1]["start"])
        edited = dict(SPANS)
        edited["shapes.coax"] = SPANS["shapes.coax"].replace("y: int;", "y:  int;")
        e = self.spans(self.tree(edited, "x") / "main.coax")
        self.assertEqual(e["Point"][0], base["Point"][0])
        self.assertNotEqual(e["Point"][1]["sha256"], base["Point"][1]["sha256"])
        self.assertEqual(e["Mode"][1]["sha256"], base["Mode"][1]["sha256"])

    def test_generated_declarations_name_their_directive(self):
        root = self.tree({"wire.coax": '#module "game.wire";\n\n#import_proto "types.pb" as Wire;\nmain :: () {}\n'})
        shutil.copy(ROOT.parent / "proto/descriptors/ios-home.pb", root / "types.pb")
        r = self.report(root / "wire.coax")
        gen = [d for d in r["declarations"] if d["generated"]]
        self.assertTrue(gen)
        for d in gen:
            self.assertIsNone(d["span"])
            self.assertEqual(d["origin"]["directive"], "#import_proto")
            self.assertEqual((Path(d["origin"]["unit"]).name, d["origin"]["line"]), ("wire.coax", 3))
        authored = next(d for d in r["declarations"] if d["name"] == "main")
        self.assertEqual(authored["origin"], "authored")
        m = self.run_coax("deps", root / "wire.coax", "--modules")
        self.assertEqual(m.returncode, 0, m.stderr)
        r = json.loads(m.stdout)
        self.assertEqual(r["target"]["name"], "native-lp64")
        self.assertTrue(all(d["span"] is None for d in r["unavailable"]["generated_declarations"]))
        self.assertTrue(all(d["span"] is None for d in r["declarations"] if d["generated"]))
        self.assertTrue(all(d["span"]["sha256"] for d in r["declarations"] if not d["generated"]))


if __name__ == "__main__":
    unittest.main()
