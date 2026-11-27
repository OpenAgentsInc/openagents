from nodes import AggNode


def compile_agg(node: AggNode) -> str:
    return f"{node.func}({node.expr})"
