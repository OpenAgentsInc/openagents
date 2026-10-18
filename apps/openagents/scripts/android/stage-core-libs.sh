#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 || ! -d "$1" ]]; then
  echo "usage: $0 CARGO_BUILD_DIR APP_JNILIBS" >&2
  exit 2
fi
if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  echo "ANDROID_NDK_HOME is required" >&2
  exit 1
fi

build_dir="$(cd "$1" && pwd -P)"
app_jni_libs="$2"
mkdir -p "$app_jni_libs"
app_jni_libs="$(cd "$app_jni_libs" && pwd -P)"
readelf="$(find -L "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt" -type f -name llvm-readelf -print -quit)"
strip="$(find -L "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt" -type f -name llvm-strip -print -quit)"
if [[ -z "$readelf" || ! -x "$readelf" || -z "$strip" || ! -x "$strip" ]]; then
  echo "llvm-readelf and llvm-strip must exist in the pinned Android NDK" >&2
  exit 1
fi

for abi in arm64-v8a x86_64; do
  case "$abi" in
    arm64-v8a)
      ndk_target="aarch64-linux-android"
      cargo_target="aarch64-linux-android"
      ;;
    x86_64)
      ndk_target="x86_64-linux-android"
      cargo_target="x86_64-linux-android"
      ;;
  esac

  source_core="$build_dir/$cargo_target/mobile/deps/libopenagents_mobile.so"
  libcxx="$(find -L "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt" \
    -path "*/sysroot/usr/lib/$ndk_target/libc++_shared.so" -type f -print -quit)"
  if [[ ! -f "$source_core" || -z "$libcxx" || ! -f "$libcxx" ]]; then
    echo "missing Android core or NDK libc++ runtime for $abi" >&2
    exit 1
  fi

  needed="$("$readelf" -d "$source_core" | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p')"
  if [[ -z "$needed" ]]; then
    echo "could not read DT_NEEDED entries from $source_core" >&2
    exit 1
  fi
  while IFS= read -r library; do
    case "$library" in
      libandroid.so|libdl.so|libm.so|libc.so|libc++_shared.so) ;;
      *)
        echo "unsupported Android runtime dependency for $abi: $library" >&2
        exit 1
        ;;
    esac
  done <<<"$needed"

  destination="$app_jni_libs/$abi"
  mkdir -p "$destination"
  find "$destination" -maxdepth 1 -type f -name '*.so' -delete
  cp "$source_core" "$destination/libopenagents_mobile.so"
  symbols_before="$("$readelf" --dyn-syms --wide "$source_core" | sed '/^$/d')"
  "$strip" --strip-debug "$destination/libopenagents_mobile.so"
  symbols_after="$("$readelf" --dyn-syms --wide "$destination/libopenagents_mobile.so" | sed '/^$/d')"
  if [[ "$symbols_before" != "$symbols_after" ]]; then
    echo "stripping changed dynamic symbols for $abi" >&2
    exit 1
  fi
  cp "$libcxx" "$destination/libc++_shared.so"
done

echo "staged stripped mobile cores and pinned NDK libc++ runtimes under $app_jni_libs"
