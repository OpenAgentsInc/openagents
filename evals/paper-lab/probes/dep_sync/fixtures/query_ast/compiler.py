# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
from nodes import AggNode


def compile_agg(node: AggNode) -> str:
    return f"{node.func}({node.expr})"
