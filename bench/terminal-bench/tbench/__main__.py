"""``python -m tbench`` mirrors the ``tbench`` console script."""

import sys

from .cli import main

sys.exit(main())
