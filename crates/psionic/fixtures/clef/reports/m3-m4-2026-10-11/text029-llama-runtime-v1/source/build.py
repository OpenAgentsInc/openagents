"""Build one static CPU-only diagnostic from the pinned public source checkout."""
import argparse
import json
import os
import subprocess
from common import HERE, UPSTREAM, require, sha, upstream_identity, write

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--output-dir', type=__import__('pathlib').Path, required=True)
    a = p.parse_args()
    require(os.environ.get('CLEF_LLAMA_GUARDED') == '1', 'owned build guard required')
    require(not a.output_dir.exists(), 'preserve prior build')
    a.output_dir.mkdir(parents=True)
    identity = upstream_identity()
    sdk = subprocess.check_output(['xcrun', '--sdk', 'macosx', '--show-sdk-path'], text=True, timeout=10).strip()
    clang = subprocess.check_output(['xcrun', '--find', 'clang'], text=True, timeout=10).strip()
    clangxx = subprocess.check_output(['xcrun', '--find', 'clang++'], text=True, timeout=10).strip()
    flags = {'CMAKE_BUILD_TYPE': 'Release', 'BUILD_SHARED_LIBS': 'OFF', 'GGML_CPU': 'ON',
             'GGML_BACKEND_DL': 'OFF', 'GGML_METAL': 'OFF', 'GGML_CUDA': 'OFF', 'GGML_VULKAN': 'OFF',
             'GGML_SYCL': 'OFF', 'GGML_OPENCL': 'OFF', 'GGML_WEBGPU': 'OFF', 'GGML_RPC': 'OFF',
             'GGML_BLAS': 'OFF', 'GGML_ACCELERATE': 'OFF', 'GGML_OPENMP': 'OFF', 'GGML_LLAMAFILE': 'OFF',
             'GGML_CPU_REPACK': 'OFF', 'GGML_CPU_KLEIDIAI': 'OFF', 'GGML_NATIVE': 'OFF', 'GGML_CCACHE': 'OFF',
             'GGML_CPU_ARM_ARCH': 'armv8.2-a+dotprod', 'GGML_CPU_ALL_VARIANTS': 'OFF',
             'LLAMA_BUILD_COMMON': 'OFF', 'LLAMA_BUILD_TOOLS': 'OFF', 'LLAMA_BUILD_MTMD': 'OFF',
             'LLAMA_BUILD_TESTS': 'OFF', 'LLAMA_BUILD_EXAMPLES': 'OFF', 'LLAMA_BUILD_SERVER': 'OFF',
             'LLAMA_CURL': 'OFF', 'CMAKE_INTERPROCEDURAL_OPTIMIZATION': 'OFF',
             'FETCHCONTENT_FULLY_DISCONNECTED': 'ON', 'CMAKE_OSX_SYSROOT': sdk,
             'CMAKE_C_COMPILER': clang, 'CMAKE_CXX_COMPILER': clangxx, 'CLEF_UPSTREAM_SOURCE': str(UPSTREAM)}
    cmake = a.output_dir / 'cmake'
    commands = [['cmake', '-S', str(HERE), '-B', str(cmake), *[f'-D{k}={v}' for k,v in flags.items()]],
                ['cmake', '--build', str(cmake), '--target', 'clef-llama-diagnostic', '--parallel', '2']]
    record = {'status': 'building', 'upstream': identity, 'commands': commands, 'flags': flags,
              'driver_sha256': sha(HERE / 'driver.cpp'), 'source': str(HERE)}
    write(a.output_dir / 'receipt.json', record)
    for command in commands:
        subprocess.run(command, check=True, timeout=590)
    require(upstream_identity() == identity, 'upstream changed during build')
    binary = cmake / 'clef-llama-diagnostic'
    require(binary.is_file(), 'diagnostic executable missing')
    links = subprocess.check_output(['otool', '-L', str(binary)], text=True, timeout=10)
    require(all(bad not in links for bad in ('Metal', 'Accelerate', 'libggml', 'libllama', '@rpath')), 'unexpected non-static/device link')
    require(sha(HERE / 'driver.cpp') == record['driver_sha256'], 'driver changed during build')
    record.update(status='complete', binary=str(binary), binary_sha256=sha(binary), linked_libraries=links,
                  compiler_version=subprocess.check_output([clangxx, '--version'], text=True, timeout=10),
                  cmake_cache_sha256=sha(cmake / 'CMakeCache.txt'))
    write(a.output_dir / 'receipt.json', record)

if __name__ == '__main__':
    main()
