// One POST /v1/systemone call, net/http only.
//
// Reads OPENAGENTS_BASE_URL and OPENAGENTS_API_KEY from the environment —
// the key stays out of every command line. Run:
//
//	go run ask.go "I was charged twice on the March invoice."
package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"
)

func main() {
	base := strings.TrimRight(os.Getenv("OPENAGENTS_BASE_URL"), "/")
	key := os.Getenv("OPENAGENTS_API_KEY")
	if base == "" || key == "" {
		fmt.Fprintln(os.Stderr, "set OPENAGENTS_BASE_URL and OPENAGENTS_API_KEY")
		os.Exit(2)
	}
	state := "I was charged twice."
	if len(os.Args) > 1 {
		state = os.Args[1]
	}

	body, _ := json.Marshal(map[string]any{
		"model": "shared-kev",
		"state": state,
		"questions": map[string]any{
			"refund": map[string]any{
				"type":         "noul",
				"instructions": "Does the customer ask for money back?",
			},
			"department": map[string]any{
				"type":         "choice",
				"instructions": "Which team should handle this request?",
				"criteria": map[string]string{
					"billing":   "Charges, invoices, and refunds",
					"technical": "Bugs and outages",
					"none":      "No team fits this request",
				},
			},
		},
	})

	request, _ := http.NewRequest(http.MethodPost, base+"/v1/systemone", bytes.NewReader(body))
	request.Header.Set("Authorization", "Bearer "+key)
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("Idempotency-Key", "example-ask-1")
	request.Header.Set("X-Attempt", "1")

	response, err := (&http.Client{Timeout: 30 * time.Second}).Do(request)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(4)
	}
	defer response.Body.Close()
	payload, _ := io.ReadAll(response.Body)
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		// Errors are typed: {"error": {"code", "message"}}.
		fmt.Fprintln(os.Stderr, string(payload))
		if response.StatusCode >= 500 {
			os.Exit(4)
		}
		os.Exit(3)
	}

	var answered struct {
		Model   string          `json:"model"`
		Answers json.RawMessage `json:"answers"`
		Usage   json.RawMessage `json:"usage"`
	}
	_ = json.Unmarshal(payload, &answered)
	fmt.Println(string(answered.Answers))
	fmt.Printf("model: %s  usage: %s\n", answered.Model, string(answered.Usage))
}
