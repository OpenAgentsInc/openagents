"""Release rejection checks using disposable app bundles, without signing."""
import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("archive", Path(__file__).with_name("archive.py"))
archive = importlib.util.module_from_spec(spec)
spec.loader.exec_module(archive)


class ArchiveChecks(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.app = Path(self.scratch.name) / "Zeron.app"
        self.app.mkdir()
        self.version = {"version": "1.1", "build": "1000"}
        self.info = {"CFBundleIdentifier": "com.openagents.coder",
                     "CFBundleShortVersionString": "1.1", "CFBundleVersion": "1000",
                     "CFBundleExecutable": "Zeron"}
        self.write_info()
        (self.app / "Zeron").write_text("clean binary strings")
        self.privacy = plistlib.loads((archive.ROOT / "apps/ios/Zeron/PrivacyInfo.xcprivacy").read_bytes())
        self.write_privacy()

    def write_info(self):
        (self.app / "Info.plist").write_bytes(plistlib.dumps(self.info))

    def write_privacy(self):
        (self.app / "PrivacyInfo.xcprivacy").write_bytes(plistlib.dumps(self.privacy))

    def test_release_metadata_passes(self):
        archive.verify(self.app, self.version)

    def test_wrong_bundle_is_rejected(self):
        self.info["CFBundleIdentifier"] = "sh.zeron.ios"
        self.write_info()
        with self.assertRaises(AssertionError):
            archive.verify(self.app, self.version)

    def test_extension_build_must_match_app(self):
        extension = self.app / "PlugIns/Voice.appex"
        extension.mkdir(parents=True)
        other = dict(self.info, CFBundleIdentifier="com.openagents.coder.LiveActivity", CFBundleVersion="999")
        (extension / "Info.plist").write_bytes(plistlib.dumps(other))
        with self.assertRaises(AssertionError):
            archive.verify(self.app, self.version)

    def test_empty_collection_disclosure_is_rejected(self):
        self.privacy["NSPrivacyCollectedDataTypes"] = []
        self.write_privacy()
        with self.assertRaises(AssertionError):
            archive.verify(self.app, self.version)

    def test_tracking_is_rejected(self):
        self.privacy["NSPrivacyTracking"] = True
        self.write_privacy()
        with self.assertRaises(AssertionError):
            archive.verify(self.app, self.version)

    def test_fixture_and_push_permission_code_are_rejected(self):
        for symbol in ("sign_in_from_fixture", "requestAuthorizationWithOptions"):
            with self.subTest(symbol=symbol):
                (self.app / "Zeron").write_text(symbol)
                with self.assertRaises(AssertionError):
                    archive.verify(self.app, self.version)


if __name__ == "__main__":
    unittest.main()
