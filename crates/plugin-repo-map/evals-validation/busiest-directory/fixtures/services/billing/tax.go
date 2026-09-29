package billing

// Tax is the tax on cents at a rate in basis points, rounded down.
func Tax(cents int64, basisPoints int64) int64 {
	return cents * basisPoints / 10000
}
