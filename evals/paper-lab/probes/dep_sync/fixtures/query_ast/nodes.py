# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
from dataclasses import dataclass
from typing import Optional


@dataclass
class AggNode:
    func: str
    expr: str
