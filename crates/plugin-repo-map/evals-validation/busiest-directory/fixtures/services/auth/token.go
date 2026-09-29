package auth

import "time"

// Token is a bearer token with an expiry.
type Token struct {
	Value   string
	Expires time.Time
}

// Valid says whether the token is still good at now.
func (t Token) Valid(now time.Time) bool {
	return now.Before(t.Expires)
}
