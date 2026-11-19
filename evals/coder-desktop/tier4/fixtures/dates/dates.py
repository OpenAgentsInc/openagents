# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
from datetime import datetime


def parse_ts(s):
    """Parse an ISO 8601 timestamp into an aware datetime in UTC."""
    return datetime.strptime(s, "%Y-%m-%dT%H:%M:%S")
