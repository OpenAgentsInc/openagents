#!/usr/bin/env bash
# Boot a fresh guest with no host network, credentials, or desktop state.
set -euo pipefail

if [[ ${1:-} != --inside ]]; then
  : "${ANDROID_HOME:?Run this check inside nix develop ./os#android}"
  state=${1:?Pass a new evidence directory under openagents scratch}
  mkdir -p "$state"
  state=$(realpath "$state")
  [[ -z $(ls -A "$state") ]] || { echo 'Use an empty evidence directory.' >&2; exit 1; }
  # A network namespace has no host interfaces or default route. This also
  # isolates adb's normal ports from every existing emulator and adb server.
  exec timeout --signal=TERM --kill-after=15s 240s \
    unshare --user --map-root-user --net bash "$0" --inside "$state"
fi

state=$2
export HOME="$state/home"
export ANDROID_USER_HOME="$state/user"
export ANDROID_EMULATOR_HOME="$state/emulator"
export ANDROID_AVD_HOME="$state/avd"
unset ADB_SERVER_SOCKET ANDROID_SERIAL
mkdir -p "$HOME" "$ANDROID_USER_HOME" "$ANDROID_EMULATOR_HOME" "$ANDROID_AVD_HOME"
ip link set lo up
ip -brief address > "$state/network.txt"
ip route >> "$state/network.txt"
[[ -z $(ip route) ]] || { echo 'Offline check has a route.' >&2; exit 1; }

emulator_pid=
# Invoked indirectly by the EXIT trap.
# shellcheck disable=SC2329
cleanup() {
  if [[ -n $emulator_pid ]]; then
    timeout 5 adb -s emulator-5580 emu kill >/dev/null 2>&1 || true
    kill "$emulator_pid" 2>/dev/null || true
    wait "$emulator_pid" 2>/dev/null || true
  fi
  adb kill-server >/dev/null 2>&1 || true
}
trap 'cleanup' EXIT
trap 'exit 130' INT TERM

emulator -accel-check > "$state/kvm.txt" 2>&1
printf 'no\n' | avdmanager create avd --name a0-offline \
  --package 'system-images;android-35;google_apis;x86_64' \
  --device pixel_7 --path "$ANDROID_AVD_HOME/a0-offline.avd" > "$state/create.txt" 2>&1
# No snapshot, audio, camera, or host GPU is needed for this boot check.
emulator -avd a0-offline -port 5580 -accel on -no-window \
  -gpu swiftshader_indirect -no-audio -no-metrics -no-boot-anim \
  -no-snapshot -memory 2048 -cores 2 -camera-back none -camera-front none \
  > "$state/emulator.log" 2>&1 &
emulator_pid=$!
adb start-server > "$state/adb.log" 2>&1
for ((attempt=0; attempt<90; attempt++)); do
  kill -0 "$emulator_pid" 2>/dev/null || { tail -40 "$state/emulator.log"; exit 1; }
  boot=$(timeout 3 adb -s emulator-5580 shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || true)
  if [[ $boot == 1 ]]; then
    {
      echo 'sys.boot_completed=1'
      adb -s emulator-5580 shell getprop ro.build.version.sdk
      adb -s emulator-5580 shell getprop ro.product.cpu.abi
      adb -s emulator-5580 shell pm list packages -3
    } > "$state/boot.txt"
    cat "$state/boot.txt"
    exit 0
  fi
  sleep 2
done
echo "Emulator did not finish booting; see $state/emulator.log" >&2
exit 1
