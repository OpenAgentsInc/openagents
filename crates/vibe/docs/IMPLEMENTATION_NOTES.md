# Implementation Notes (Vibe Infra UI)

Quick notes to track UI scaffolding decisions for the Vibe infra/billing surface.

- **Mocks first, clear pathways later**: all infra usage, billing, and plan data are currently stubbed (`VibeSnapshot` mock). Actions mutate in-memory state to simulate Cloudflare control plane + billing responses.
- **No border radius rule**: UI styling avoids `border-radius` per repo hook.
- **Action state**: `ActionState` tracks provisioning/refreshing/paying/downloading, disables buttons, and shows inline status text. Replace with real async wiring once APIs are ready.
- **Usage metrics**: `UsageMetric` carries `limit` and `remaining` fields. Infra panel shows pills + breakdown grid.
- **Plan summary**: Displays plan limits with detail labels and upgrade/manage placeholders.
- **Invoice panel**: Shows invoice id/status with download/pay stubs that log events.
- **Next wiring steps**:
  - Replace snapshot mocks with control-plane APIs (DO/R2/D1/KV) and billing backend.
  - Map action state to real async calls and surface error/success banners.
  - Compute `remaining` from actual usage vs plan limits; handle overages.
  - Add loading indicators to resource bar based on real fetch state.
