#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around the Coax compiler's checks).
set -euo pipefail
coax/bin/coax test coax/native/updates-policy.coax
coax/bin/coax mutate coax/native/updates-policy.coax --root-only
coax/bin/coax test coax/native/updates-wire.coax
coax/bin/coax test coax/native/starter/app.coax
coax/web/tool.sh coax/native/profile.coax
