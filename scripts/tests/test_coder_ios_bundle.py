"""Exercise the iOS packaging boundary without an Apple SDK or signing key."""

import importlib.util
import plistlib
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "verify-coder-ios-bundle.py"
SPEC = importlib.util.spec_from_file_location("coder_ios_bundle", SCRIPT)
bundle_check = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = bundle_check
SPEC.loader.exec_module(bundle_check)


class BundleChecks(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.bundle = (Path(self.directory.name) / "Coder.app").resolve()
        self.bundle.mkdir()
        (self.bundle / "Info.plist").write_bytes(plistlib.dumps({
            "CFBundleExecutable": "Coder", "CFBundleIdentifier": "com.openagents.coder",
            "CFBundleShortVersionString": "0.5.0", "CFBundleVersion": "45",
        }))
        self.metadata = {}
        self.signed = []
        self.image("Coder", ["/usr/lib/libSystem.B.dylib"])

    def image(self, name, dependencies=(), rpaths=()):
        path = self.bundle / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"\xcf\xfa\xed\xfe")
        self.metadata[path.resolve()] = bundle_check.Image(tuple(dependencies), tuple(rpaths))
        return path

    def verify(self):
        return bundle_check.verify_bundle(
            self.bundle, inspect=lambda path: self.metadata[path.resolve()],
            signature=lambda path: self.signed.append(path),
        )

    def test_system_only_static_app(self):
        result = self.verify()
        self.assertEqual(result["status"], "passed")
        self.assertEqual(self.signed, [self.bundle, self.bundle / "Coder"])

    def test_build44_absolute_rust_dylib_is_rejected(self):
        self.image("Coder", ["/Users/builder/target/aarch64-apple-ios/release/deps/libcoder_mobile.dylib"])
        with self.assertRaisesRegex(bundle_check.BundleError, "must link libcoder_mobile.a"):
            self.verify()

    def test_other_build_machine_path_is_rejected(self):
        self.image("Coder", ["/Users/builder/libhelper.dylib"])
        with self.assertRaisesRegex(bundle_check.BundleError, "Nonportable library dependency"):
            self.verify()

    def test_signed_embedded_transitive_rpath_library(self):
        self.image("Coder", ["@rpath/A.framework/A"], ["@executable_path/Frameworks"])
        self.image("Frameworks/A.framework/A", ["@rpath/libB.dylib"])
        self.image("Frameworks/libB.dylib", ["/System/Library/Frameworks/Metal.framework/Metal"])
        self.assertEqual(len(self.verify()["images"]), 3)
        self.assertEqual(len(self.signed), 4)

    def test_missing_embedded_library_is_rejected(self):
        self.image("Coder", ["@rpath/libMissing.dylib"], ["@executable_path/Frameworks"])
        with self.assertRaisesRegex(bundle_check.BundleError, "Missing bundled library"):
            self.verify()

    def test_unused_embedded_library_is_also_checked(self):
        self.image("Frameworks/libUnused.dylib", ["/tmp/libMissing.dylib"])
        with self.assertRaisesRegex(bundle_check.BundleError, "Nonportable library dependency"):
            self.verify()

    def test_host_runpath_is_rejected_even_if_unused(self):
        self.image("Coder", ["/usr/lib/libSystem.B.dylib"], ["/Users/builder/target"])
        with self.assertRaisesRegex(bundle_check.BundleError, "Nonportable library search path"):
            self.verify()

    def test_loader_path_cannot_escape_bundle(self):
        self.image("Coder", ["@loader_path/../outside.dylib"])
        with self.assertRaisesRegex(bundle_check.BundleError, "escapes the app bundle"):
            self.verify()

    def test_symlink_cannot_escape_bundle(self):
        outside = self.bundle.parent / "outside.dylib"
        outside.write_bytes(b"\xcf\xfa\xed\xfe")
        (self.bundle / "outside.dylib").symlink_to(outside)
        with self.assertRaisesRegex(bundle_check.BundleError, "symlink escapes"):
            self.verify()

    def test_system_path_traversal_is_not_trusted(self):
        self.image("Coder", ["/usr/lib/../../Users/builder/libhelper.dylib"])
        with self.assertRaisesRegex(bundle_check.BundleError, "Nonportable library dependency"):
            self.verify()

    def test_load_command_parser_ignores_dylib_id_but_keeps_weak_load(self):
        image = bundle_check.parse_load_commands("""
Load command 0
          cmd LC_ID_DYLIB
      cmdsize 80
         name @rpath/Own.framework/Own (offset 24)
Load command 1
          cmd LC_LOAD_WEAK_DYLIB
      cmdsize 80
         name /usr/lib/swift/libswiftCore.dylib (offset 24)
Load command 2
          cmd LC_RPATH
      cmdsize 40
         path @loader_path/Frameworks (offset 12)
""")
        self.assertEqual(image.dependencies, ("/usr/lib/swift/libswiftCore.dylib",))
        self.assertEqual(image.rpaths, ("@loader_path/Frameworks",))


if __name__ == "__main__":
    unittest.main()
