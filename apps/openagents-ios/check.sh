#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around otool, nm, strings and find: fails if any disallowed framework, resource, or legacy symbol is present).
# Enforces the OpenAgents iOS Metal GPU invariants:
#   - No JavaScriptCore / WebKit frameworks and no .js / .wasm / .html / .cnu resources
#   - Shared Components/components.metal shader, Geist.ttf font, and bip39-english.txt wordlist are present
#   - No legacy removed AppCore/JS/WebView symbols
#   apps/openagents-ios/check.sh [PATH/OpenAgents.app]
set -euo pipefail
app="${1:-$(dirname "$0")/build/Build/Products/Debug-iphonesimulator/OpenAgents.app}"
[ -d "$app" ] || { echo "check: no app at $app"; exit 2; }
bad=0
while IFS= read -r b; do
  [ -n "$b" ] || continue
  if otool -L "$b" 2>/dev/null | grep -E 'JavaScriptCore|WebKit'; then
    echo "check: $b links a JavaScript or web framework"; bad=1
  fi
  if nm -U "$b" 2>/dev/null | grep -E 'NativeProgram|NativeControls|NativeChrome|NativeMessage|NativeTool|NativeWorkspace|NativeUpdates|NativeWorldHost|NativeArcadeSurface|JSContext|WKWebView|chat_frame|oc_open|oa_core_|AppRelaySocket|ForemanScreen|ProjectScreen' | head -5 | grep .; then
    echo "check: $b has symbols of the removed legacy AppCore/WebView system"; bad=1
  fi
done < <(find "$app" -type f \( -name OpenAgents -o -name '*.dylib' -o -path '*.framework/*' \) -perm +111 2>/dev/null; find "$app" -name '*.debug.dylib' 2>/dev/null)
if find "$app" -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.html' -o -name '*.cnu' \) | grep .; then
  echo "check: the app contains JavaScript, WebAssembly, HTML, or update resources"; bad=1
fi
[ -f "$app/Components/components.metal" ] || { echo "check: missing Components/components.metal shader"; bad=1; }
[ -f "$app/Components/fonts/Geist.ttf" ] || { echo "check: missing Components/fonts/Geist.ttf font"; bad=1; }
[ -f "$app/bip39-english.txt" ] || { echo "check: missing bip39-english.txt wordlist"; bad=1; }
[ "$bad" = 0 ] && echo "check: clean OpenAgents iOS binary (Metal + Geist atlas + Spark Wallet + Account/Cloud Chat, zero JS/WebKit/legacy AppCore) in $app"
exit "$bad"
