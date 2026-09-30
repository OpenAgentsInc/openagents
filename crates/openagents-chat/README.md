# OpenAgents chat

The shared hosted chat implementation used by the phone and the desktop host.
It retains the phone's NIP-CJ conversation reader, authenticated relay connection,
chat router metadata, encrypted cache, and conversation lifecycle. The caller
owns its signing key, storage directory, Tokio runtime, and wake callback.

The desktop window receives chat data over the host's same-user control socket.
It does not receive signing keys. No model API key ships with either app.
See [the chat worker](../../docs/deployment/chat-worker.md) for the service,
wire kinds, and quotas.

`basic_coder::Door` admits an injected offline implementation for tests.
`Relay::with_wake` notifies on partials; `BasicChats::with_wake` notifies
when jobs finish. Phone modules re-export this implementation, and
`coder_computers::cache` re-exports the same encrypted store, preserving
existing cache paths and wire fields.

Run the isolated acceptance tests with:

```sh
cargo test --locked -p openagents-chat
```

The ignored live test uses a fresh temporary identity and saves no chats.
