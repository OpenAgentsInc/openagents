from dataclasses import dataclass
from typing import Optional


@dataclass
class AggNode:
    func: str
    expr: str
