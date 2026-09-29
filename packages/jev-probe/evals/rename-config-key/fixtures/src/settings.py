"""Settings read from config.toml."""

import tomllib


def load(path="config.toml"):
    with open(path, "rb") as file:
        raw = tomllib.load(file)
    return {
        "host": raw["server"]["host"],
        "port": raw["server"]["port"],
        "server_timeout": raw["server"]["timeout_secs"],
        "client_timeout": raw["client"]["timeout_secs"],
        "retries": raw["client"]["retries"],
    }
