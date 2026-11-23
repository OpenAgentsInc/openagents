#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around otool, nm, strings and find: fails if OpenAgents macOS links disallowed web/JS/GPU/relay/account symbols or URLs).
# Enforces clean-sheet Antigravity-over-Tailscale invariants for OpenAgents macOS (GitHub #13993):
#   apps/openagents-macos/check.sh [PATH/OpenAgentsMac.app]
set -euo pipefail
app="${1:-$(dirname "$0")/build/Build/Products/Debug/OpenAgentsMac.app}"
[ -d "$app" ] || { echo "check: no app at $app"; exit 2; }
bad=0
bins=$(find "$app/Contents/MacOS" -type f -perm +111 2>/dev/null)
for b in $bins; do
  if otool -L "$b" 2>/dev/null | grep -E 'JavaScriptCore|WebKit'; then
    echo "check: $b links a JavaScript or web framework"; bad=1
  fi
  if nm -U "$b" 2>/dev/null | grep -E 'NativeProgram|NativeControls|NativeChrome|JSContext|WKWebView|chat_frame|oc_open|oa_core_|AppRelaySocket|AppAccount|AppAuth|KitPasskey|ForemanScreen|ProjectScreen' | head -5 | grep .; then
    echo "check: $b has symbols of the removed GPU/relay/account system"; bad=1
  fi
  if strings "$b" 2>/dev/null | grep -E 'relay\.openagents\.com|webcredentials:openagents\.com|https://openagents\.com' | head -5 | grep .; then
    echo "check: $b contains a forbidden public relay or cloud account URL"; bad=1
  fi
done
if find "$app" -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.html' -o -name '*.cnu' -o -name '*.metal' \) | grep .; then
  echo "check: the app contains JavaScript, WebAssembly, HTML, update, or Metal shader resources"; bad=1
fi
[ "$bad" = 0 ] && echo "check: clean OpenAgents macOS binary (no JS/WebKit/Metal, no legacy GPU/AppCore/account/relay symbols or URLs) in $app"
exit "$bad"
