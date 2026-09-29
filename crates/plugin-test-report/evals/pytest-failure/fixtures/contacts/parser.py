import unicodedata


def parse(name):
    first, last = name.split(" ", 1)
    return type("Name", (), {"first": first, "last": unicodedata.normalize("NFKD", last).encode("ascii", "ignore").decode()})
