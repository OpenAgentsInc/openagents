# The Wasm toolchain for this machine, sourced by wasm.sh: sets
# COAXIAL_CLANG, COAXIAL_SYSROOT and COAXIAL_RESOURCES and puts wasm-ld on
# PATH, unless they are already set.
#   macOS: Homebrew's llvm, lld, wasi-libc and wasi-runtimes
#          (brew install llvm lld wasi-libc wasi-runtimes)
#   Linux: clang 21, lld, wasi-libc and compiler-rt from nixpkgs, linked into
#          ${COAXIAL_TOOLCHAIN:-~/.cache/coax-wasm-toolchain} on first use
case "$(uname -s)" in
  Darwin)
    COAXIAL_CLANG="${COAXIAL_CLANG:-/opt/homebrew/opt/llvm/bin/clang}"
    COAXIAL_SYSROOT="${COAXIAL_SYSROOT:-/opt/homebrew/opt/wasi-libc/share/wasi-sysroot}"
    COAXIAL_RESOURCES="${COAXIAL_RESOURCES:-/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes}"
    PATH="/opt/homebrew/opt/lld/bin:$PATH"
    ;;
  Linux)
    coaxial_tc="${COAXIAL_TOOLCHAIN:-$HOME/.cache/coax-wasm-toolchain}"
    if [[ -z "${COAXIAL_CLANG:-}" && ! -x "$coaxial_tc/bin/clang" ]]; then
      echo "toolchain.sh: linking the Wasm toolchain from nixpkgs into $coaxial_tc" >&2
      out() { nix build --no-link --print-out-paths "$@"; }
      clang="$(out nixpkgs#llvmPackages_21.clang-unwrapped)"
      lld="$(out nixpkgs#llvmPackages_21.lld)"
      libc="$(out nixpkgs#pkgsCross.wasi32.wasilibc)"
      libc_dev="$(out nixpkgs#pkgsCross.wasi32.wasilibc.dev)"
      rt="$(out nixpkgs#pkgsCross.wasi32.llvmPackages_21.compiler-rt)"
      mkdir -p "$coaxial_tc/bin" "$coaxial_tc/sysroot/lib" "$coaxial_tc/resource/lib/wasm32-unknown-wasip1"
      ln -sfn "$clang/bin/clang" "$coaxial_tc/bin/clang"
      ln -sfn "$lld/bin/wasm-ld" "$coaxial_tc/bin/wasm-ld"
      ln -sfn "$libc_dev/include" "$coaxial_tc/sysroot/include"
      ln -sfn "$libc/lib/wasm32-wasip1" "$coaxial_tc/sysroot/lib/wasm32-wasip1"
      ln -sfn "$("$clang/bin/clang" -print-resource-dir)/include" "$coaxial_tc/resource/include"
      ln -sfn "$rt/lib/wasi/libclang_rt.builtins-wasm32.a" "$coaxial_tc/resource/lib/wasm32-unknown-wasip1/libclang_rt.builtins.a"
    fi
    COAXIAL_CLANG="${COAXIAL_CLANG:-$coaxial_tc/bin/clang}"
    COAXIAL_SYSROOT="${COAXIAL_SYSROOT:-$coaxial_tc/sysroot}"
    COAXIAL_RESOURCES="${COAXIAL_RESOURCES:-$coaxial_tc/resource}"
    PATH="$coaxial_tc/bin:$PATH"
    ;;
esac
export COAXIAL_CLANG COAXIAL_SYSROOT COAXIAL_RESOURCES PATH
