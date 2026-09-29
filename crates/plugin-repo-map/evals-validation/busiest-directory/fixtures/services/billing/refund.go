package billing

// Refund reverses a paid invoice and returns the ledger entry to post.
func Refund(invoice Invoice) (Entry, bool) {
	if !invoice.Paid {
		return Entry{}, false
	}
	return Entry{Account: invoice.Customer, Cents: -invoice.Cents}, true
}
