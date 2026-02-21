# Web Shell JS Host Boundary

Status: active
Owner: `owner:openagents.com`
Issue: OA-RUST-024

## Rule

JavaScript in the web shell is host glue only. Product logic authority is Rust.

## JS Responsibilities (Allowed)

1. Load wasm bundle and start Rust entrypoint.
2. Handle browser-only bootstrap concerns (status text, canvas mount wiring).
3. Register service worker.
4. Poll static build manifest (`/manifest.json`) for build-skew detection and trigger SW update promotion.
5. Evaluate browser GPU capability policy (`webgpu`/`webgl2`/`limited`) and pass host mode hint to wasm startup.
6. Surface bootstrap/update failure visibly in browser console/DOM.

## JS Responsibilities (Prohibited)

1. Routing logic (route selection, route-state reducers, feature routes).
2. Auth/session/token logic.
3. Product/business state management.
4. API calls for command/read logic (`/api/*` and runtime business endpoints).
5. Persistent product state in browser storage.

## Enforcement

Run:

```bash
apps/openagents.com/web-shell/check-host-shim.sh
```

The boundary check fails if host shim JS includes prohibited primitives/keywords.

## Files in Scope

- Host shim JS: `apps/openagents.com/web-shell/host/host-shim.js`
- Capability policy JS: `apps/openagents.com/web-shell/host/capability-policy.js`
- Rust wasm entrypoint: `apps/openagents.com/web-shell/src/lib.rs`
