#!/usr/bin/env bash
# Check build prerequisites with synthetic files, without building the app.
set -euo pipefail
state=${1:?Pass an evidence directory under openagents scratch}
mkdir -p "$state"
state=$(realpath "$state")
: "${ANDROID_HOME:?Run inside nix develop ./os#android}"
: "${ANDROID_NDK_HOME:?The Android shell must provide an NDK}"
{
  java -version 2>&1
  gradle --version
  cargo ndk --version
  adb version
  emulator -version
  cat "$ANDROID_NDK_HOME/source.properties"
  cat "$ANDROID_HOME/build-tools/35.0.0/source.properties"
  "$ANDROID_HOME/cmake/3.22.1/bin/cmake" --version
} > "$state/versions.txt" 2>&1

printf 'extern "C" int a0_probe() { return 79; }\n' > "$state/probe.cpp"
toolchain="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
for target in aarch64-linux-android x86_64-linux-android; do
  "$toolchain/${target}26-clang++" -shared -fPIC -Wl,-z,max-page-size=16384 \
    "$state/probe.cpp" -o "$state/$target.so"
  "$toolchain/llvm-readelf" -h -l "$state/$target.so" > "$state/$target.elf.txt"
done

mkdir -p "$state/gradle-project"
printf "rootProject.name = 'a0-tools-probe'\n" > "$state/gradle-project/settings.gradle"
printf "tasks.register('probe') { doLast { println 'A0 Gradle OK' } }\n" > "$state/gradle-project/build.gradle"
gradle --offline --no-daemon --max-workers=2 -p "$state/gradle-project" probe > "$state/gradle.txt" 2>&1
cat "$state/gradle.txt"
