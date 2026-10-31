#!/usr/bin/env python3
"""Logical modules through the real driver (spec 17.1, r1a-logical-declarations).

ModuleIdentity: `#module` owners, #load membership and the nominal
declaration IDs that `coax deps --declarations` reports from the checked graph.
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
        # Identity is not a namespace: one graph keeps one flat declaration namespace.
        both = self.tree({"l.coax": '#module "math.left";\nVec :: struct { x: f32; }\n',
                          "r.coax": '#module "math.right";\nVec :: struct { x: f32; }\n',
                          "main.coax": '#load "l.coax";\n#load "r.coax";\nmain :: () {}\n'}, "both")
        p = self.run_coax("check", both / "main.coax")
        self.assertNotEqual(p.returncode, 0, "two same-spelling declarations must still collide in the flat namespace")

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
        self.assertIn("declaration_ranges", un)
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


if __name__ == "__main__":
    unittest.main()
