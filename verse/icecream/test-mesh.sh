#!/usr/bin/env bash
# Current Coax models and the repaired independent C++ oracle copy. External
# sources remain read only; original mesh-oracle.coax remains historical.
# --table retains a new labelled table in the work directory, never overwrites
# the historical C++ fixture. AETHYR_DEMO/COAX/CC/CXX retain their old meanings.
set -euo pipefail
cd "$(dirname "$0")/../.."
exec python3 verse/icecream/check_mesh_bands.py "$@"
