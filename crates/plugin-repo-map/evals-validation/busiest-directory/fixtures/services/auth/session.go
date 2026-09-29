package auth

// Session ties a token to a user.
type Session struct {
	User  string
	Token Token
}
