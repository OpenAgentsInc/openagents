# Configure Coder's decision client

Conversation classification and program decisions use the same configuration
resolver and the existing Jev Rust SDK. Generation and executor settings are
separate. This is the HTTP/local portion of #9502; relay profiles, discovery,
receipt consumption, and dynamic authorization integration remain open.

Set `CODER_DECISION_PROFILE` to select an explicit profile:

| Profile | Required settings | Behavior |
| --- | --- | --- |
| `local` | `CODER_DECISION_URL`, `CODER_DECISION_MODEL` | Credential-free direct loopback HTTP. The SDK refuses remote addresses, proxies, redirects, credential headers, and URL credentials. |
| `http` | URL, model, and `CODER_DECISION_KEY` | A keyed Decision API endpoint. Use the endpoint's authorized model ID and bearer key. |
| `provider` | URL, model, and key | An explicitly selected compatible provider using the same native HTTP contract. |

The URL and model settings in every row are `CODER_DECISION_URL` and
`CODER_DECISION_MODEL`. The local profile refuses `CODER_DECISION_KEY`; it
never fabricates a provider credential. Use a loopback IP literal, such as
`http://127.0.0.1:8080`, rather than a hostname. This restricts the client
connection; it does not attest what computation the local server performs.

An explicit profile takes precedence over legacy `TYPESAFE_*` configuration.
Without a profile, Coder preserves `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL`, and
`TYPESAFE_DEFAULT_MODEL`. If none is configured, ordinary chat has no classifier.
A legacy URL or model without a key is an error. Explicit decision settings
without a profile, unknown profile names, missing required values, and malformed
configuration are errors rather than an absent classifier. Non-Unicode values
also refuse configuration.

The agent reports configuration errors at startup. A separately constructed
program runtime retains the configuration error and refuses decision steps with
`door_configuration`. Tests that explicitly inject a client override the
configuration result. An optional unconfigured chat path does not gain shell or
program authority from the missing classifier.

Store keys in a protected environment file outside the checkout. Do not place
keys in programs, prompts, traces, or issue bodies. The profiles do not introduce
fallback destinations. The remaining transport, cancellation, receipt, and
required-router behavior must be verified through the broader #9502 contract.

The headless regression tests use a local HTTP fixture to verify the selected
model and absence of an Authorization header despite an inherited provider key.
They also verify that malformed profiles stop startup. The terminal uses the
same agent opener; interactive terminal rendering is not covered by those tests.
