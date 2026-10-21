#!/usr/bin/env bash
# Source from a web build script after defining web, target, and target_dir.
# Save each backend before Cargo replaces the package's final output.
#
# Cargo and wasm-bindgen run one at a time, since every build writes the
# same target directory. Each module's `wasm-opt` and budget check then run
# in the background, at most WEB_BUILD_JOBS at once (default: the CPU
# count), because each takes about 25 s on one core and a page builds dozens
# of modules (#12417). Call wait_renderers before packaging what
# build_renderers wrote; it fails when any background job failed.
web_render_pids=""
web_render_count=0
web_render_failed=0

web_render_jobs() {
  if [ -n "${WEB_BUILD_JOBS:-}" ]; then
    echo "$WEB_BUILD_JOBS"
  else
    getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4
  fi
}

# Wait for the oldest background job, recording a failure.
web_render_reap() {
  local oldest="${web_render_pids%% *}"
  wait "$oldest" || web_render_failed=1
  web_render_pids="${web_render_pids#"$oldest"}"
  web_render_pids="${web_render_pids# }"
  web_render_count=$((web_render_count - 1))
}

# Optimize and check one module (run in the background). The names-carrying
# Cargo output is copied first, so an over-budget report reads this module's
# build, not a later one.
web_render_finish() { # DIRECTORY MODULE NAMES_DIR
  local directory="$1" module="$2" names="$3"
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$directory/${module}_bg.wasm" -o "$directory/${module}_bg.wasm" &&
  WEB_BUILD_NAMES_DIR="$names" \
    python3 "$web/../scripts/package-web-build.py" --check "$directory" "${module}_bg.wasm"
}

build_renderers() { # PACKAGE NAME OUTDIR
  local package="$1" name="$2" directory="$3" backend module names
  mkdir -p "$directory"
  for backend in webgpu webgl; do
    module="$name"
    [ "$backend" != webgl ] || module="${name}_webgl"
    cargo build --locked --manifest-path "$web/Cargo.toml" -p "$package" \
      --release --target "$target" --no-default-features --features "$backend"
    wasm-bindgen --target web --no-typescript --out-name "$module" --out-dir "$directory" \
      "$target_dir/$target/release/$name.wasm"
    names="$(mktemp -d)"
    cp "$target_dir/$target/release/$name.wasm" "$names/$module.wasm"
    while [ "$web_render_count" -ge "$(web_render_jobs)" ]; do web_render_reap; done
    (
      set +e
      web_render_finish "$directory" "$module" "$names"
      status=$?
      rm -rf "$names"
      exit "$status"
    ) &
    web_render_pids="${web_render_pids:+$web_render_pids }$!"
    web_render_count=$((web_render_count + 1))
  done
  cp "$web/gpui-web/renderer.js" "$directory/renderer.js"
}

wait_renderers() {
  while [ "$web_render_count" -gt 0 ]; do web_render_reap; done
  if [ "$web_render_failed" -ne 0 ]; then
    web_render_failed=0
    echo "a web module failed to optimize or is over its budget (above)" >&2
    return 1
  fi
}
