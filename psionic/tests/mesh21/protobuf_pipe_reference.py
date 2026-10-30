#!/usr/bin/env python3
"""Optional test-only official-protobuf observer; never a product dependency.

Reads one synthetic demo witness from the private pipe, compares its actual
native writer bytes with protobuf's deterministic serialization, and forwards
it to the real checker through another private pipe. No witness/key bytes are
printed or persisted. The only file output is redacted qualification status.
Use the repository's pinned protobuf 5.29.3 tooling interpreter.
"""
import argparse
import json
import signal
import subprocess
import sys
from pathlib import Path
from google.protobuf import __version__ as protobuf_version
from google.protobuf.descriptor_pb2 import FileDescriptorSet
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message_factory import GetMessageClass

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--descriptor", required=True)
    parser.add_argument("--record", required=True)
    parser.add_argument("--checker", required=True)
    args, checker_args = parser.parse_known_args()
    signal.alarm(30)
    if protobuf_version != "5.29.3":
        raise RuntimeError("pinned protobuf tooling required")
    wire = sys.stdin.buffer.read(262145)
    if not 0 < len(wire) <= 262144:
        raise RuntimeError("bounded private witness required")
    descriptors = FileDescriptorSet.FromString(Path(args.descriptor).read_bytes())
    pool = DescriptorPool()
    for item in descriptors.file:
        pool.Add(item)
    cls = GetMessageClass(pool.FindMessageTypeByName(
        "openagents.psionic.mesh.v1alpha1.PrivateReplayWitness"))
    message = cls.FromString(wire)
    if any(not message.HasField(field.name) for field in message.DESCRIPTOR.fields):
        raise RuntimeError("required witness presence")
    if len(message.request_message_keys) != 76 or len(message.result_message_keys) != 76:
        raise RuntimeError("message opening lengths")
    if message.version != "openagents.psionic.mesh-witness.v1":
        raise RuntimeError("witness version")
    if message.SerializeToString(deterministic=True) != wire:
        raise RuntimeError("native/reference canonical encoding mismatch")
    status = {"reference": "official protobuf 5.29.3, protoc 29.3 descriptor",
              "required_fields": 13, "canonical_roundtrip": True,
              "opening_lengths": [76, 76], "private_bytes_persisted": False}
    Path(args.record).write_text(json.dumps(status, indent=2) + "\n")
    child = subprocess.run([args.checker, *checker_args], input=wire,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20)
    # The real checker has the public-key/public-label-only output contract.
    sys.stdout.buffer.write(child.stdout)
    sys.stderr.buffer.write(child.stderr)
    return child.returncode

if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        # Do not stringify exceptions or locals containing a private message.
        print("reference observer: validation failed", file=sys.stderr)
        sys.exit(2)
