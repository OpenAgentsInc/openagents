package billing

// Invoice is one bill for one customer.
type Invoice struct {
	ID       string
	Customer string
	Cents    int64
	Paid     bool
}

// Total sums the open invoices.
func Total(invoices []Invoice) int64 {
	var sum int64
	for _, invoice := range invoices {
		if !invoice.Paid {
			sum += invoice.Cents
		}
	}
	return sum
}
