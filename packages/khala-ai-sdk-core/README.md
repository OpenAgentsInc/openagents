# Khala AI SDK Core

OpenCode-style AI SDK Core adapter for Khala Code.

This package keeps AI SDK Core as provider-call transport only. It calls a
`streamText`-compatible function, maps stream parts into
`openagents.khala_runtime_event.v1`, and bridges Khala tools into AI SDK
`tool()` definitions while executing through the OpenAgents/Khala tool
dispatcher.

The package does not fork AI SDK Core and does not make AI SDK stream parts the
product transcript schema.
