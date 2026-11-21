#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper caching and compiling pinned Breez Spark Swift SDK modules for macOS; holds no logic)
# Prepares ~/.cache/openagents/breez-sdk-spark/0.27.2/macos-arm64/ containing:
#   - breez_sdk_sparkFFI.framework (from pinned 0.27.2 xcframework, SHA-256 verified)
#   - BigNumber.swiftmodule + libBigNumber.a
#   - BreezSdkSpark.swiftmodule + libBreezSdkSpark.a
# Prints the macos-arm64 cache directory on stdout.
set -euo pipefail

VER="0.27.2"
FFI_URL="https://github.com/breez/breez-sdk-spark-swift/releases/download/${VER}/breez_sdk_sparkFFI.xcframework.zip"
FFI_SHA="b4ce00f470076abe9e7dcf3f69ae706327bc63ffe43f48065473a0f915065209"
SWIFT_ZIP_URL="https://github.com/breez/breez-sdk-spark-swift/archive/refs/tags/${VER}.tar.gz"
BIGINT_ZIP_URL="https://github.com/mkrd/Swift-BigInt/archive/refs/tags/v2.4.0.tar.gz"

CACHE_ROOT="${OA_SPARK_SDK_CACHE:-$HOME/.cache/openagents/breez-sdk-spark/$VER}"
OUT="$CACHE_ROOT/macos-arm64"
SRC="$CACHE_ROOT/src"
STAMP="$OUT/.built-macos14-arm64-v1"

if [[ -f "$STAMP" && -d "$OUT/breez_sdk_sparkFFI.framework" && -f "$OUT/libBreezSdkSpark.a" && -f "$OUT/libBigNumber.a" ]]; then
  echo "$OUT"
  exit 0
fi

mkdir -p "$OUT" "$SRC"

# Reuse local Xcode SourcePackages checkout when available to avoid redundant downloads.
REPO_ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
MAIN_REPO="$(cd "$(git -C "$REPO_ROOT" rev-parse --git-common-dir)/.." 2>/dev/null && pwd || echo "$REPO_ROOT")"
LOCAL_SPM="$MAIN_REPO/apps/openagents-ios/build-device/SourcePackages"

if [[ ! -d "$OUT/breez_sdk_sparkFFI.framework" ]]; then
  LOCAL_FW="$LOCAL_SPM/artifacts/breez-sdk-spark-swift/breez_sdk_sparkFFI/breez_sdk_sparkFFI.xcframework/macos-arm64_x86_64/breez_sdk_sparkFFI.framework"
  if [[ -d "$LOCAL_FW" ]]; then
    cp -R "$LOCAL_FW" "$OUT/breez_sdk_sparkFFI.framework"
  else
    ZIP="$SRC/breez_sdk_sparkFFI.xcframework.zip"
    curl -fsSL "$FFI_URL" -o "$ZIP"
    ACTUAL="$(shasum -a 256 "$ZIP" | awk '{print $1}')"
    if [[ "$ACTUAL" != "$FFI_SHA" ]]; then
      echo "spark-sdk.sh: SHA-256 mismatch for breez_sdk_sparkFFI.xcframework.zip (got $ACTUAL)" >&2
      rm -f "$ZIP"
      exit 1
    fi
    rm -rf "$SRC/xcframework"
    mkdir -p "$SRC/xcframework"
    ditto -x -k "$ZIP" "$SRC/xcframework"
    cp -R "$SRC/xcframework/breez_sdk_sparkFFI.xcframework/macos-arm64_x86_64/breez_sdk_sparkFFI.framework" "$OUT/breez_sdk_sparkFFI.framework"
  fi
fi

if [[ ! -f "$SRC/breez_sdk_spark.swift" || ! -f "$SRC/breez_sdk_spark_bindings.swift" ]]; then
  LOCAL_BREEZ="$LOCAL_SPM/checkouts/breez-sdk-spark-swift/Sources/BreezSdkSpark"
  if [[ -f "$LOCAL_BREEZ/breez_sdk_spark.swift" && -f "$LOCAL_BREEZ/breez_sdk_spark_bindings.swift" ]]; then
    cp "$LOCAL_BREEZ/breez_sdk_spark.swift" "$LOCAL_BREEZ/breez_sdk_spark_bindings.swift" "$SRC/"
  else
    curl -fsSL "$SWIFT_ZIP_URL" | tar -xz -C "$SRC"
    cp "$SRC/breez-sdk-spark-swift-${VER}/Sources/BreezSdkSpark/breez_sdk_spark.swift" \
       "$SRC/breez-sdk-spark-swift-${VER}/Sources/BreezSdkSpark/breez_sdk_spark_bindings.swift" "$SRC/"
  fi
fi

if [[ ! -f "$SRC/Swift-Big-Number-Core.swift" ]]; then
  LOCAL_BIGINT="$LOCAL_SPM/checkouts/Swift-BigInt/Sources/BigNumber/Swift-Big-Number-Core.swift"
  if [[ -f "$LOCAL_BIGINT" ]]; then
    cp "$LOCAL_BIGINT" "$SRC/"
  else
    curl -fsSL "$BIGINT_ZIP_URL" | tar -xz -C "$SRC"
    cp "$SRC/Swift-BigInt-2.4.0/Sources/BigNumber/Swift-Big-Number-Core.swift" "$SRC/"
  fi
fi

# 1. Compile BigNumber module
xcrun -sdk macosx swiftc -O -whole-module-optimization -swift-version 5 -suppress-warnings -parse-as-library \
  -target arm64-apple-macos14.0 \
  -module-name BigNumber -emit-module -emit-module-path "$OUT/BigNumber.swiftmodule" \
  -emit-object -o "$OUT/BigNumber.o" \
  "$SRC/Swift-Big-Number-Core.swift"
ar rcs "$OUT/libBigNumber.a" "$OUT/BigNumber.o"

# 2. Compile BreezSdkSpark module (breez_sdk_spark.swift + breez_sdk_spark_bindings.swift)
xcrun -sdk macosx swiftc -O -whole-module-optimization -swift-version 5 -suppress-warnings -parse-as-library \
  -target arm64-apple-macos14.0 \
  -F "$OUT" -I "$OUT" \
  -module-name BreezSdkSpark -emit-module -emit-module-path "$OUT/BreezSdkSpark.swiftmodule" \
  -emit-object -o "$OUT/BreezSdkSpark.o" \
  "$SRC/breez_sdk_spark.swift" "$SRC/breez_sdk_spark_bindings.swift"
ar rcs "$OUT/libBreezSdkSpark.a" "$OUT/BreezSdkSpark.o"

touch "$STAMP"
echo "$OUT"
