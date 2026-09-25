class Ledger:
    """Posts entries and retries a failed post."""

    RETRY_LIMIT = 3

    def __init__(self):
        self.entries = []

    def post(self, entry):
        self.entries.append(entry)

    def retry_limit(self):
        return self.RETRY_LIMIT

    def posted(self):
        return len(self.entries)
