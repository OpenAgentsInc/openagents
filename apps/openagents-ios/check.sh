#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around otool, nm and find: it fails when any of them prints a match).
# The app holds no JavaScript path (GitHub #13645): fails if the built .app links JavaScriptCore or
# WebKit, contains a .js, .wasm or .html resource, or has a symbol of the removed UIKit projection.
#   apps/openagents-ios/check.sh [PATH/OpenAgents.app]
set -euo pipefail
app="${1:-$(dirname "$0")/build/Build/Products/Debug-iphonesimulator/OpenAgents.app}"
[ -d "$app" ] || { echo "check: no app at $app"; exit 2; }
bad=0
bins=$(find "$app" -type f \( -name OpenAgents -o -name '*.dylib' -o -path '*.framework/*' \) -perm +111 2>/dev/null; find "$app" -name '*.debug.dylib')
for b in $bins; do
  if otool -L "$b" 2>/dev/null | grep -E 'JavaScriptCore|WebKit'; then echo "check: $b links a JavaScript or web framework"; bad=1; fi
  if nm -U "$b" 2>/dev/null | grep -E 'NativeProgram|NativeControls|NativeChrome|NativeMessage|NativeTool|NativeWorkspace|NativeUpdates|NativeWorldHost|NativeArcadeSurface|JSContext|WKWebView' | head -5 | grep .; then echo "check: $b has symbols of the removed system"; bad=1; fi
done
if find "$app" -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.html' -o -name '*.cnu' \) | grep .; then echo "check: the app contains JavaScript, WebAssembly, HTML or update resources"; bad=1; fi
[ "$bad" = 0 ] && echo "check: no JavaScriptCore, no WebKit, no .js/.wasm/.html, no symbol of the removed system in $app"
exit "$bad"
