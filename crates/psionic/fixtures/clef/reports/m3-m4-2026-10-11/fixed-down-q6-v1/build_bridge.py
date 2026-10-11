#!/usr/bin/env python3
"""Build the small CPU quantizer bridge after a separate build handoff."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REVISION = "62a6f74bab748b1755f4cd8dfc0871caf865c6c8"


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def git(source, *args):
    return subprocess.check_output(["git", "-C", str(source), *args], text=True, timeout=10).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    resources = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    require(os.environ.get("CLEF_Q6_REGEN_GUARDED") == "1", "bridge build requires the owned-process guard")
    require({"build", "memory", "disk"} <= resources and not ({"gpu", "quiet"} & resources), "requires build, memory, and disk leases only")
    source = args.source.resolve()
    require(git(source, "rev-parse", "HEAD") == REVISION, "upstream source is not the pinned revision")
    require(not git(source, "status", "--porcelain", "--untracked-files=all"), "upstream checkout must be clean, including untracked files")
    pins = json.loads((HERE / "upstream.json").read_text())
    for path, record in pins["files"].items():
        require(digest(source / path) == record["sha256"], "upstream source hash differs")
    require(not args.output_dir.exists(), "preserve existing build evidence")
    args.output_dir.mkdir(parents=True)
    output = args.output_dir.resolve()
    cmake = output / "cmake"
    configure = ["cmake", "-S", str(source), "-B", str(cmake), "-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=ON",
                 "-DGGML_CPU=OFF", "-DGGML_METAL=OFF", "-DGGML_CUDA=OFF", "-DGGML_VULKAN=OFF", "-DGGML_SYCL=OFF",
                 "-DGGML_BLAS=OFF", "-DGGML_ACCELERATE=OFF", "-DGGML_OPENMP=OFF", "-DGGML_NATIVE=OFF", "-DGGML_CCACHE=OFF",
                 "-DLLAMA_BUILD_COMMON=OFF", "-DLLAMA_BUILD_TOOLS=OFF", "-DLLAMA_BUILD_MTMD=OFF",
                 "-DLLAMA_BUILD_TESTS=OFF", "-DLLAMA_BUILD_EXAMPLES=OFF", "-DLLAMA_BUILD_SERVER=OFF"]
    build = ["cmake", "--build", str(cmake), "--target", "ggml-base", "--parallel", "2"]
    manifest = dict(status="building", upstream_revision=REVISION, source=str(source),
                    source_files=pins["files"], bridge_source_sha256=digest(HERE / "q6_bridge.cpp"), commands=[configure, build])
    manifest_path = output / "bridge.json"
    try:
        subprocess.run(configure, check=True, timeout=120)
        subprocess.run(build, check=True, timeout=300)
        libraries = sorted({p.resolve() for p in cmake.rglob("libggml-base.dylib")})
        require(len(libraries) == 1, "expected one macOS CPU ggml-base library")
        library = libraries[0]
        bridge = output / "q6_bridge.dylib"
        compiler = subprocess.check_output(["xcrun", "--find", "clang++"], text=True, timeout=10).strip()
        sdk = subprocess.check_output(["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True, timeout=10).strip()
        require(Path(sdk).is_dir(), "macOS SDK path is unavailable")
        manifest["sdk_path"] = sdk
        compile_command = [compiler, "-isysroot", sdk, "-O2", "-std=c++17", "-fno-fast-math", "-ffp-contract=off", "-dynamiclib",
                           str(HERE / "q6_bridge.cpp"), "-I", str(source / "ggml/include"), "-L", str(library.parent),
                           "-lggml-base", "-Wl,-rpath," + str(library.parent), "-o", str(bridge)]
        manifest["commands"].append(compile_command)
        subprocess.run(compile_command, check=True, timeout=60)
        require(not git(source, "status", "--porcelain", "--untracked-files=all"), "upstream checkout changed during build")
        require(git(source, "rev-parse", "HEAD") == REVISION, "upstream revision changed during build")
        for path, record in pins["files"].items():
            require(digest(source / path) == record["sha256"], "upstream source hash changed during build")
        linked = subprocess.check_output(["otool", "-L", str(bridge), str(library)], text=True, timeout=10)
        require("Metal.framework" not in linked and "Accelerate.framework" not in linked, "unexpected GPU/math backend linked")
        manifest.update(status="built", bridge=str(bridge), bridge_sha256=digest(bridge),
                        runtime_libraries={str(library): digest(library)}, linked_libraries=linked,
                        compiler_version=subprocess.check_output([compiler, "--version"], text=True, timeout=10))
    except BaseException as error:
        manifest.update(status="failed", error=str(error), error_type=type(error).__name__)
        raise
    finally:
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"status": "built", "manifest": str(manifest_path), "bridge_sha256": manifest["bridge_sha256"]}))


if __name__ == "__main__":
    main()
