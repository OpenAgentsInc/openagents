from billing import Ledger


def main():
    ledger = Ledger()
    ledger.post({"amount": 5})
    print("postgres is not used here")
