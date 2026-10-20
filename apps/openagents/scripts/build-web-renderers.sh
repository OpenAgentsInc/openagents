#!/usr/bin/env bash
# Source from a web build script after defining web, target, and target_dir.
# Save each backend before Cargo replaces the package's final output.
build_renderers() { # PACKAGE NAME OUTDIR
  local package="$1" name="$2" directory="$3" backend module
  mkdir -p "$directory"
  for backend in webgpu webgl; do
    module="$name"
    [ "$backend" != webgl ] || module="${name}_webgl"
    cargo build --locked --manifest-path "$web/Cargo.toml" -p "$package" \
      --release --target "$target" --no-default-features --features "$backend"
    wasm-bindgen --target web --no-typescript --out-name "$module" --out-dir "$directory" \
      "$target_dir/$target/release/$name.wasm"
    wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
      --enable-sign-ext --enable-mutable-globals --enable-reference-types \
      "$directory/${module}_bg.wasm" -o "$directory/${module}_bg.wasm"
    # Check each backend before the next build replaces its named Cargo output.
    WEB_BUILD_NAMES_DIR="$target_dir/$target/release" \
      python3 "$web/../scripts/package-web-build.py" --check "$directory" "${module}_bg.wasm"
  done
  cp "$web/gpui-web/renderer.js" "$directory/renderer.js"
}
