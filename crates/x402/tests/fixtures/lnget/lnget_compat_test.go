// The lnget side of crates/x402's `lnget_built_credential_is_accepted`.
//
// Copy this file into a checkout of github.com/lightninglabs/lnget at
// mpp/, point OA_FIXTURE at this directory, and run
//
//	OA_FIXTURE=$PWD go test ./mpp -run TestOpenAgentsFront -v
//
// It hands the front's real WWW-Authenticate value to lnget's
// Handler.HandleChallenge with a payer that returns the fixture preimage,
// writes the Authorization value lnget builds to authorization.txt, and
// reads the front's Payment-Receipt with lnget's ParseReceipt.
package mpp

import (
	"context"
	"encoding/hex"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/lightninglabs/lnget/l402"
	"github.com/lightningnetwork/lnd/lntypes"
	"github.com/lightningnetwork/lnd/lnwire"
)

type fixturePayer struct {
	preimage lntypes.Preimage
	invoice  string
}

func (p *fixturePayer) PayInvoice(_ context.Context, invoice string,
	_ int64, _ time.Duration) (*l402.PaymentResult, error) {

	p.invoice = invoice
	return &l402.PaymentResult{
		Preimage:   p.preimage,
		AmountPaid: lnwire.MilliSatoshi(21000),
	}, nil
}

func TestOpenAgentsFront(t *testing.T) {
	dir := os.Getenv("OA_FIXTURE")
	if dir == "" {
		t.Skip("OA_FIXTURE is not set")
	}
	read := func(name string) string {
		data, err := os.ReadFile(filepath.Join(dir, name))
		if err != nil {
			t.Fatal(err)
		}
		return strings.TrimSpace(string(data))
	}

	raw, err := hex.DecodeString(read("preimage.txt"))
	if err != nil {
		t.Fatal(err)
	}
	var preimage lntypes.Preimage
	copy(preimage[:], raw)
	payer := &fixturePayer{preimage: preimage}

	header := http.Header{}
	header.Add("WWW-Authenticate", read("challenge.txt"))
	resp := &http.Response{StatusCode: http.StatusPaymentRequired, Header: header}
	if !IsPaymentChallenge(resp) {
		t.Fatal("lnget does not see a Payment challenge")
	}

	handler := NewHandler(&HandlerConfig{
		Payer:          payer,
		MaxCostSat:     21,
		MaxFeeSat:      1,
		PaymentTimeout: time.Second,
	})
	result, err := handler.HandleChallenge(context.Background(), resp, "api.example.com")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(payer.invoice, "lnbc210n1") {
		t.Fatalf("paid %q", payer.invoice)
	}
	if result.Credential.HeaderName != "Authorization" {
		t.Fatalf("header %q", result.Credential.HeaderName)
	}
	err = os.WriteFile(filepath.Join(dir, "authorization.txt"),
		[]byte(result.Credential.HeaderValue+"\n"), 0o644)
	if err != nil {
		t.Fatal(err)
	}

	receipt, err := ParseReceipt(read("receipt.txt"))
	if err != nil {
		t.Fatal(err)
	}
	challenge, err := ParseChallenge(read("challenge.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if receipt.Status != "success" || receipt.Method != "lightning" ||
		receipt.ChallengeID != challenge.ID ||
		receipt.Reference != challenge.Request.MethodDetails.PaymentHash {
		t.Fatalf("receipt %+v", receipt)
	}
}
