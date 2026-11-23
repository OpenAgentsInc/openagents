#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around otool, nm, strings and find: fails if any disallowed framework, resource, symbol, or relay URL is present).
# Enforces the GPU Antigravity-over-Tailscale mobile invariants (GitHub #14023):
#   - No JavaScriptCore / WebKit frameworks and no .js / .wasm / .html / .cnu resources
#   - Only the shared Components/components.metal shader is permitted
#   - No legacy AppCore, public relay, cloud account, Passkey, Foreman, or Projects symbols
#   - No public relay (relay.openagents.com) or cloud account URLs in the binary
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
  if nm -U "$b" 2>/dev/null | grep -E 'NativeProgram|NativeControls|NativeChrome|NativeMessage|NativeTool|NativeWorkspace|NativeUpdates|NativeWorldHost|NativeArcadeSurface|JSContext|WKWebView|chat_frame|oc_open|oa_core_|AppRelaySocket|AppAccount|AppAuth|KitPasskey|AppIdentity|AppLibrary|AppProvider|AppWorkspace|ForemanScreen|ProjectScreen' | head -5 | grep .; then
    echo "check: $b has symbols of the removed legacy AppCore/relay/account system"; bad=1
  fi
  if strings "$b" 2>/dev/null | grep -E 'relay\.openagents\.com|webcredentials:openagents\.com|https://openagents\.com' | head -5 | grep .; then
    echo "check: $b contains a forbidden public relay or cloud account URL"; bad=1
  fi
done < <(find "$app" -type f \( -name OpenAgents -o -name '*.dylib' -o -path '*.framework/*' \) -perm +111 2>/dev/null; find "$app" -name '*.debug.dylib' 2>/dev/null)
if find "$app" -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.html' -o -name '*.cnu' \) | grep .; then
  echo "check: the app contains JavaScript, WebAssembly, HTML, or update resources"; bad=1
fi
[ -f "$app/Components/components.metal" ] || { echo "check: missing Components/components.metal shader"; bad=1; }
[ -f "$app/Components/fonts/Geist.ttf" ] || { echo "check: missing Components/fonts/Geist.ttf font"; bad=1; }
[ "$bad" = 0 ] && echo "check: clean GPU Antigravity-over-Tailscale binary (Metal + Geist atlas, zero JS/WebKit/AppCore/account/relay symbols or URLs) in $app"
exit "$bad"
