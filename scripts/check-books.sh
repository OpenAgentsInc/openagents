#!/usr/bin/env bash
# Owned book delivery entry point. Metadata is fast; qualify one bounded route
# at a time with --route/--receipt, retaining unchanged evidence with --previous.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $# -gt 0 && $1 != --check ]]; then
    exec python3 scripts/book_routes.py "$@"
fi
[[ $# -eq 0 || ( $# -eq 1 && $1 == --check ) ]] || exit 2
python3 coax/tests/book_registry.py
python3 coax/book/check-structure.py
python3 psionic/tests/book.py
python3 psionic/tests/book.py --book aethyr
python3 psionic/tests/book.py --book nostr
python3 scripts/book_routes.py --check
