"""Exercise backup collection when deletion wins the source-file open."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "deploy/backup/nostr-relay-backup"


class BackupMediaTest(unittest.TestCase):
    def run_backup(self, missing=False, corrupt=False):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            media, output, binaries = [root / name for name in ("media", "output", "bin")]
            for path in (media, output, binaries, media / ".deleted"):
                path.mkdir()
            (media / ".tmp").mkdir()
            (media / ".tmp" / "upload").write_bytes(b"unfinished upload")
            content = b"a blob moved while backup opens it"
            digest = hashlib.sha256(content).hexdigest()
            key = "b" * 64
            live = media / digest[:2] / f"{digest}.{key}"
            live.parent.mkdir()
            if not missing:
                live.write_bytes(b"damaged" if corrupt else content)
            sql = root / "rows.sql"
            sql.write_text(f"COPY media_blob FROM stdin;\n{digest}\t{key}\t1\ttext/plain\t0\tt\n\\.\n")
            programs = {
                "pg_dump": '#!/bin/sh\nfor arg do case "$arg" in --file=*) touch "${arg#--file=}";; esac; done\n',
                "pg_restore": '#!/bin/sh\nfor arg do case "$arg" in --file=*) cat "$TEST_ROWS" > "${arg#--file=}";; esac; done\n',
                # Force the rename immediately before cp opens the live path.
                "cp": '#!/bin/sh\nif test "$2" = "$TEST_LIVE" && test -f "$2"; then mv "$2" "$TEST_RETAINED"; fi\nexec "$TEST_CP" "$@"\n',
            }
            for name, text in programs.items():
                path = binaries / name
                path.write_text(text)
                path.chmod(0o700)
            env = dict(os.environ, PATH=f"{binaries}:{os.environ['PATH']}",
                       NOSTR_RELAY_BACKUP_DIR=str(output), NOSTR_RELAY_MEDIA_ROOT=str(media),
                       NOSTR_RELAY_BACKUP_DATABASE="unused", TEST_ROWS=str(sql),
                       TEST_LIVE=str(live), TEST_RETAINED=str(media / ".deleted" / live.name),
                       TEST_CP=shutil.which("cp"))
            result = subprocess.run([str(SCRIPT)], env=env, capture_output=True, timeout=15)
            if missing or corrupt:
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(digest, result.stderr.decode())
                self.assertEqual(list(output.iterdir()), [])
            else:
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                self.assertEqual(len(list(output.glob("*.manifest"))), 1)
                with tarfile.open(next(output.glob("*.tar"))) as archive:
                    self.assertFalse(any(".tmp" in name for name in archive.getnames()))
                    self.assertEqual(archive.extractfile(f"./{digest[:2]}/{live.name}").read(), content)
                self.assertFalse(live.exists())

    def test_deletion_between_lookup_and_open_keeps_the_blob(self):
        self.run_backup()

    def test_missing_blob_publishes_no_manifest(self):
        self.run_backup(missing=True)

    def test_corrupt_blob_publishes_no_manifest(self):
        self.run_backup(corrupt=True)
