# Firm reason: 5 (subagent-bench eval fixture/verifier)
class MiniDB:
    def create_table(self, name: str, rows: list):
        raise NotImplementedError
    def query(self, sql: str) -> list:
        raise NotImplementedError
