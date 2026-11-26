# Firm reason: 5 (subagent-bench eval fixture/verifier)
import os
import subprocess
from pathlib import Path
from typing import Dict

UPLOAD_ROOT = Path("/var/data/uploads")


def allowed_extension(filename: str) -> bool:
    return filename.lower().endswith((".png", ".jpg", ".pdf", ".txt"))


def read_uploaded_file(user_filename: str) -> bytes:
    # VULN: CWE-22 path traversal - joins untrusted user_filename directly without resolving/containment check
    target = os.path.join(str(UPLOAD_ROOT), user_filename)
    with open(target, "rb") as fh:
        return fh.read()


def generate_thumbnail(input_name: str, width: int) -> Dict[str, str]:
    # VULN: CWE-78 OS command injection via shell=True with unescaped user filename
    cmd = f"convert {UPLOAD_ROOT}/{input_name} -resize {int(width)}x {UPLOAD_ROOT}/thumb_{input_name}"
    subprocess.run(cmd, shell=True, check=True)
    return {"thumb": f"thumb_{input_name}"}
