package billing

// Entry is one movement in the ledger.
type Entry struct {
	Account string
	Cents   int64
}

// Balance is the sum of an account's entries.
func Balance(entries []Entry, account string) int64 {
	var sum int64
	for _, entry := range entries {
		if entry.Account == account {
			sum += entry.Cents
		}
	}
	return sum
}
