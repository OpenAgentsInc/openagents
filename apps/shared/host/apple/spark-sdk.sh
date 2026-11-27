#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper caching and compiling pinned Breez Spark Swift SDK modules for macOS and iOS; holds no logic)
# Modes:
#   spark-sdk.sh                       -> builds macOS arm64 static libs/modules and prints cache dir
#   spark-sdk.sh --ios-package <dest>  -> stages a local SwiftPM package (breez_sdk_sparkFFI.xcframework + BigNumber + BreezSdkSpark) for Xcode iOS/Simulator builds
set -euo pipefail

MODE="macos"
IOS_PKG_DEST=""
if [[ "${1:-}" == "--ios-package" ]]; then
  MODE="ios-package"
  IOS_PKG_DEST="${2:?missing destination directory for --ios-package}"
fi

VER="0.27.2"
FFI_URL="https://github.com/breez/breez-sdk-spark-swift/releases/download/${VER}/breez_sdk_sparkFFI.xcframework.zip"
FFI_SHA="b4ce00f470076abe9e7dcf3f69ae706327bc63ffe43f48065473a0f915065209"
SWIFT_ZIP_URL="https://github.com/breez/breez-sdk-spark-swift/archive/refs/tags/${VER}.tar.gz"
BIGINT_ZIP_URL="https://github.com/mkrd/Swift-BigInt/archive/refs/tags/v2.4.0.tar.gz"

CACHE_ROOT="${OA_SPARK_SDK_CACHE:-$HOME/.cache/openagents/breez-sdk-spark/$VER}"
OUT="$CACHE_ROOT/macos-arm64"
SRC="$CACHE_ROOT/src"
STAMP="$OUT/.built-macos14-arm64-v1"

if [[ "$MODE" == "macos" && -f "$STAMP" && -d "$OUT/breez_sdk_sparkFFI.framework" && -f "$OUT/libBreezSdkSpark.a" && -f "$OUT/libBigNumber.a" ]]; then
  echo "$OUT"
  exit 0
fi

mkdir -p "$OUT" "$SRC"

# Reuse local Xcode SourcePackages checkout when available to avoid redundant downloads.
REPO_ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
MAIN_REPO="$(cd "$(git -C "$REPO_ROOT" rev-parse --git-common-dir)/.." 2>/dev/null && pwd || echo "$REPO_ROOT")"
LOCAL_SPM="$MAIN_REPO/apps/openagents-ios/build-device/SourcePackages"

ensure_xcframework() {
  if [[ -d "$SRC/xcframework/breez_sdk_sparkFFI.xcframework" ]]; then
    return 0
  fi
  LOCAL_XCFW="$LOCAL_SPM/artifacts/breez-sdk-spark-swift/breez_sdk_sparkFFI/breez_sdk_sparkFFI.xcframework"
  if [[ -d "$LOCAL_XCFW" ]]; then
    mkdir -p "$SRC/xcframework"
    cp -R "$LOCAL_XCFW" "$SRC/xcframework/breez_sdk_sparkFFI.xcframework"
    return 0
  fi
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
}

if [[ "$MODE" == "ios-package" || ! -d "$OUT/breez_sdk_sparkFFI.framework" ]]; then
  ensure_xcframework
  if [[ ! -d "$OUT/breez_sdk_sparkFFI.framework" && -d "$SRC/xcframework/breez_sdk_sparkFFI.xcframework/macos-arm64_x86_64/breez_sdk_sparkFFI.framework" ]]; then
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

if [[ "$MODE" == "ios-package" ]]; then
  mkdir -p "$IOS_PKG_DEST/Sources/BigNumber" "$IOS_PKG_DEST/Sources/BreezSdkSpark"
  if [[ ! -d "$IOS_PKG_DEST/breez_sdk_sparkFFI.xcframework" ]]; then
    cp -R "$SRC/xcframework/breez_sdk_sparkFFI.xcframework" "$IOS_PKG_DEST/breez_sdk_sparkFFI.xcframework"
  fi
  cp -f "$SRC/Swift-Big-Number-Core.swift" "$IOS_PKG_DEST/Sources/BigNumber/Swift-Big-Number-Core.swift"
  cp -f "$SRC/breez_sdk_spark.swift" "$IOS_PKG_DEST/Sources/BreezSdkSpark/breez_sdk_spark.swift"
  cp -f "$SRC/breez_sdk_spark_bindings.swift" "$IOS_PKG_DEST/Sources/BreezSdkSpark/breez_sdk_spark_bindings.swift"
  chmod u+w "$IOS_PKG_DEST/Sources/BigNumber/Swift-Big-Number-Core.swift" \
            "$IOS_PKG_DEST/Sources/BreezSdkSpark/breez_sdk_spark.swift" \
            "$IOS_PKG_DEST/Sources/BreezSdkSpark/breez_sdk_spark_bindings.swift"
  cat > "$IOS_PKG_DEST/Package.swift" <<'EOF'
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SparkSDK",
    platforms: [.iOS(.v16), .macOS(.v14)],
    products: [
        .library(name: "BreezSdkSpark", targets: ["BreezSdkSpark"]),
        .library(name: "BigNumber", targets: ["BigNumber"]),
    ],
    targets: [
        .binaryTarget(
            name: "breez_sdk_sparkFFI",
            path: "breez_sdk_sparkFFI.xcframework"
        ),
        .target(
            name: "BigNumber",
            path: "Sources/BigNumber"
        ),
        .target(
            name: "BreezSdkSpark",
            dependencies: ["breez_sdk_sparkFFI", "BigNumber"],
            path: "Sources/BreezSdkSpark",
            swiftSettings: [.unsafeFlags(["-suppress-warnings"])]
        ),
    ]
)
EOF
  echo "$IOS_PKG_DEST"
  exit 0
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
