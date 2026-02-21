# Runtime NetworkPolicy Hardening

`runtime` enforces ingress restrictions via:

- `apps/runtime/deploy/k8s/base/networkpolicy-ingress.yaml`

## Allowed ingress

1. **BEAM distribution + EPMD**
   - TCP `9000` (distribution)
   - TCP `4369` (epmd)
   - Source: runtime peers only (`app=runtime`)

2. **Internal HTTP API**
   - TCP `4000`
   - Source must be one of:
     - same-namespace pods with `app.kubernetes.io/name=openagents-com`
     - namespaces labeled `openagents.io/control-plane=true` with pods labeled `app.kubernetes.io/name=openagents-com`
     - pods labeled `openagents.io/runtime-http-client=true` (used by smoke jobs)

## Required label contracts

- Control-plane namespaces that need runtime HTTP access must set:
  - `openagents.io/control-plane=true`
- Pods requiring runtime HTTP access outside the default Laravel selectors can set:
  - `openagents.io/runtime-http-client=true`

## Validation

- Render manifests:
  - `kubectl kustomize apps/runtime/deploy/k8s/base`
- Verify policy applied:
  - `kubectl -n <ns> get networkpolicy runtime-ingress -o yaml`
- Verify smoke pod label compatibility:
  - `kubectl -n <ns> get job runtime-smoke -o yaml`
