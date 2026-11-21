#!/usr/bin/env bash
# Firm reason: 5 (thin build/run wrapper caching and launching the Swift Breez Spark settlement bridge; holds no logic)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
spark_sdk="$("$repo/apps/shared/host/apple/spark-sdk.sh")"
wordlist="$repo/metaverse/stage-resources/bip39-english.txt"

key="$(
  shasum -a 256 \
    "$repo/pay/wallet.coax" \
    "$repo/pay/wallet-backup.coax" \
    "$repo/apps/shared/host/apple/wallet-core.h" \
    "$repo/apps/shared/host/apple/BreezKey.swift" \
    "$repo/apps/shared/host/apple/BIP39.swift" \
    "$repo/apps/shared/host/apple/WalletKeychain.swift" \
    "$repo/apps/shared/host/apple/SparkWallet.swift" \
    "$here/spark-settle-helper.swift" \
    "$spark_sdk/libBreezSdkSpark.a" | shasum -a 256 | cut -c1-32
)"

cache="${OA_SPARK_HELPER_CACHE:-$HOME/.cache/openagents/spark-settle-helper}"
bin="$cache/spark-settle-helper-$key"

if [[ ! -x "$bin" ]]; then
  mkdir -p "$cache"
  work="$(mktemp -d /tmp/oa-spark-helper.XXXXXX)"
  trap 'rm -rf "$work"' EXIT

  "$coax" c "$repo/pay/wallet.coax" --library --header "$work/pay-wallet.h" > "$work/pay-wallet.c"
  "$coax" c "$repo/pay/wallet-backup.coax" --library --header "$work/pay-wallet-backup.h" > "$work/pay-wallet-backup.c"
  cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -w)
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/pay-wallet.c" -o "$work/pay-wallet-all.o"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/pay-wallet-backup.c" -o "$work/pay-wallet-backup-all.o"
  ld -r "$work/pay-wallet-all.o" -exported_symbol '_metaverse_wallet_*' -o "$work/pay-wallet.o"
  ld -r "$work/pay-wallet-backup-all.o" -exported_symbol '_metaverse_backup_*' -exported_symbol '_metaverse_limit_*' -o "$work/pay-wallet-backup.o"

  xcrun -sdk macosx swiftc -O -swift-version 5 -suppress-warnings -parse-as-library \
    -target arm64-apple-macos14.0 \
    -import-objc-header "$repo/apps/shared/host/apple/wallet-core.h" \
    -I "$spark_sdk" -F "$spark_sdk" \
    "$repo/apps/shared/host/apple/BreezKey.swift" \
    "$repo/apps/shared/host/apple/BIP39.swift" \
    "$repo/apps/shared/host/apple/WalletKeychain.swift" \
    "$repo/apps/shared/host/apple/SparkWallet.swift" \
    "$here/spark-settle-helper.swift" \
    "$work/pay-wallet.o" "$work/pay-wallet-backup.o" \
    "$spark_sdk/libBreezSdkSpark.a" "$spark_sdk/libBigNumber.a" \
    -framework breez_sdk_sparkFFI -Xlinker -rpath -Xlinker "$spark_sdk" \
    -framework CryptoKit -framework Security -framework Foundation \
    -o "$work/spark-settle-helper"
  mv -f "$work/spark-settle-helper" "$bin"
fi

if [[ "${1:-}" == "--build-only" ]]; then
  echo "$bin"
  exit 0
fi

exec "$bin" "$@" --wordlist "$wordlist"
