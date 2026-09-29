def export(rows):
    return "\n".join(",".join(r.values()) for r in rows)
