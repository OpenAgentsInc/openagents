package notify

// Email is one message to send.
type Email struct {
	To      string
	Subject string
	Body    string
}
