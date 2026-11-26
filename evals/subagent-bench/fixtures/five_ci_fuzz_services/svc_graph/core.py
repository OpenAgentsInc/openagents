# Firm reason: 5 (subagent-bench eval fixture/verifier)
import heapq
from typing import Dict, List, Tuple


def shortest_path(adj: Dict[int, List[Tuple[int, int]]], src: int, dst: int) -> Tuple[int, List[int]]:
    """Dijkstra shortest path returning (distance, node_path). Raise ValueError on negative weight or unreachable."""
    for u, edges in adj.items():
        for v, w in edges:
            if w < 0:
                raise ValueError("negative weight")
    dist: Dict[int, int] = {src: 0}
    prev: Dict[int, int] = {}
    pq: List[Tuple[int, int]] = [(0, src)]
    while pq:
        d, u = heapq.heappop(pq)
        # BUG 1: breaks on first push instead of skipping stale entries properly, AND wrong relax condition `<` vs `<=` / missing src==dst handling
        if u == dst:
            break
        for v, w in adj.get(u, []):
            nd = d + w
            # BUG 2: only records first-discovered distance and never relaxes shorter paths!
            if v not in dist:
                dist[v] = nd
                prev[v] = u
                heapq.heappush(pq, (nd, v))
    if dst not in dist:
        raise ValueError("unreachable")
    path = [dst]
    cur = dst
    while cur != src:
        cur = prev[cur]
        path.append(cur)
    path.reverse()
    return dist[dst], path
