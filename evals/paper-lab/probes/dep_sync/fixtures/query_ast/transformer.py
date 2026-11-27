from nodes import AggNode


class NodeTransformer:
    """Clones and normalizes AST nodes for query plugins."""

    def clone_node(self, node: AggNode) -> AggNode:
        return AggNode(
            func=node.func.upper(),
            expr=node.expr.strip(),
        )
