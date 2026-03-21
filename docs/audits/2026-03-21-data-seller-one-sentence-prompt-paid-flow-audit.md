# Data Seller One-Sentence Prompt Paid Flow Audit

Date: 2026-03-21

## Scope

This audit covers the exact seller-first flow `docs/v02.md` says we are about
to hand to users:

- open the `Data Seller` pane
- give it a simple one-sentence instruction about what local data to sell
- let the seller lane normalize that into a real listing draft
- preview and publish the asset and grant
- have a buyer buy it
- deliver the payload
- consume the delivery locally and verify the bytes

This pass also includes a fresh run of the repo-owned paid headless E2E harness
to make sure the standard regression path still passes after the testability
hook added for this audit.

## Short Answer

Yes. The user-facing seller prompt flow works for listing an example dataset for
sale, and the full paid purchase path also works locally with local
`nexus-control`.

The exact prompt-driven paid proof run is recorded under:

- `target/data-market-ui-audit-2026-03-21/`
- `target/data-market-ui-audit-2026-03-21/prompt-flow-summary.json`
- `target/data-market-ui-audit-2026-03-21/consumed-dataset-from-prompt/payload`

The fresh standard paid headless regression run is recorded under:

- `target/headless-data-market-e2e-2026-03-21-audit/`
- `target/headless-data-market-e2e-2026-03-21-audit/summary.json`

## Why This Was Tested Through Desktop Control

The product promise in `docs/v02.md` is specifically a conversational `Data
Seller` pane flow. That is what I tested.

However, this terminal environment could not reliably automate the live macOS
window surface:

- `osascript` window queries timed out
- `screencapture` could not capture the display

So I added a narrow app-owned control action that sends prompt text into the
existing dedicated `Data Seller` Codex lane. That does not create a second
publication path. It exercises the same seller thread, the same seller skills,
and the same typed market tools that the pane uses.

Added surfaces:

- `autopilotctl data-market seller-prompt "<prompt>"`
- `DesktopControlActionRequest::SendDataMarketSellerPrompt`

## Local Environment

This proof did not use remote hosted infrastructure.

It used:

- local relay: `ws://127.0.0.1:54247`
- local `nexus-control`: `http://127.0.0.1:54256`
- isolated seller home:
  `target/data-market-ui-audit-2026-03-21/seller-home`
- isolated buyer home:
  `target/data-market-ui-audit-2026-03-21/buyer-home`
- seller desktop-control manifest:
  `target/data-market-ui-audit-2026-03-21/seller-desktop-control.json`
- buyer desktop-control manifest:
  `target/data-market-ui-audit-2026-03-21/buyer-desktop-control.json`

The buyer wallet was prefunded with `100 sats` before the priced request:

- `target/data-market-ui-audit-2026-03-21/buyer-wallet-status-prefunded.json`

## Example Dataset

I created a tiny local example dataset at:

- `target/data-market-ui-audit-2026-03-21/example-dataset`

It contains:

- `README.md`
- `rows.csv`

## Exact Seller Prompt

This is the exact one-sentence seller instruction used for the audit:

```text
In the Data Seller pane, turn /Users/christopherdavid/code/openagents/target/data-market-ui-audit-2026-03-21/example-dataset into a saleable listing titled 'UI Audit Example Dataset' for 5 sats targeted to npub13kkvmtjd3hlqhe0zjn0ckk6t7f90767fmgglfgtkfx0tgszcd26qzwsnhc using the targeted_request policy, then preview before publish and ask me only if anything essential is missing.
```

Command used:

```bash
target/debug/autopilotctl \
  --manifest target/data-market-ui-audit-2026-03-21/seller-desktop-control.json \
  --json data-market seller-prompt "<prompt>"
```

## What The Seller Lane Did

The prompt landed on the dedicated seller thread and the seller lane used the
real typed Data Market tools to normalize the draft.

Successful populated seller snapshot:

- `target/data-market-ui-audit-2026-03-21/prompt-poll/seller-status-37.json`

Important facts from that snapshot:

- title: `UI Audit Example Dataset`
- asset kind: `conversation_bundle`
- content digest:
  `sha256:4f84c5ed88f44eb8f661b7357be52328965eed6251e89144b029b64bf7516384`
- provenance ref:
  `oa://local-packages/ui-audit-example-dataset/4f84c5ed88f44eb8f661b7357be52328965eed6251e89144b029b64bf7516384`
- price hint: `5 sats`
- default policy: `targeted_request`
- no readiness blockers

Prompt normalization timing:

- poll iterations until ready: `37`
- poll interval: `2 seconds`
- total wait: about `74 seconds`

Codex-side logs in `target/data-market-ui-audit-2026-03-21/seller-ui.stdout.log`
showed the seller lane actually calling:

- `openagents_data_market_draft_asset`
- `openagents_data_market_draft_grant`

## Prompt-Driven Paid Proof

After the seller lane prepared the draft, I completed the rest of the product
flow in the same local run:

1. publish the asset from the prompt-derived seller draft
2. preview and publish the targeted grant
3. bring the seller online for request intake
4. refresh the buyer market and publish the targeted request
5. request the payment quote
6. wait for settled payment
7. prepare and issue the delivery bundle
8. wait for the buyer result
9. consume the delivered payload locally
10. compare the consumed payload to the original source files

Key final proof artifacts:

- `target/data-market-ui-audit-2026-03-21/publish-asset-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/preview-grant-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/publish-grant-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/buyer-request-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/request-payment-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/seller-payment-settled-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/issue-delivery-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/buyer-result-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/consume-delivery-from-prompt.json`
- `target/data-market-ui-audit-2026-03-21/prompt-flow-summary.json`

Successful final IDs from the prompt-driven run:

- asset id:
  `data_asset.npub1vp0f47hhkh2j34s338n3y0d55ata6swv5fgd9v9wd3lk63ygapdqzywkg5.conversation_bundle.UI_Audit_Example_Dataset.sha256_4f84c5ed88f44eb8f661b7357be52328965eed6251e89144b029b64bf7516384`
- grant id:
  `access_grant.npub1vp0f47hhkh2j34s338n3y0d55ata6swv5fgd9v9wd3lk63ygapdqzywkg5.data_asset.npub1vp0f47hhkh2j34s338n3y0d55ata6swv5fgd9v9wd3lk63ygapdqzywkg5.conversation_bundle.UI_Audit_Example_Dataset.sha256_4f84c5ed88f44eb8f661b7357be52328965eed6251e89144b029b64bf7516384.targeted_request.npub13kkvmtjd3hlqhe0zjn0ckk6t7f90767fmgglfgtkfx0tgszcd26qzwsnhc`
- request id:
  `dddc6da8a823666708f17b9f1d05c0d554839caec8e48325e111213490c0d739`
- seller payment pointer:
  `019d0f2d-3309-77c7-991e-b256c7f3325c`
- buyer payment pointer:
  `019d0f2d-2d54-7902-803f-dd169849bb32`
- result event id:
  `f8ab862913f3adb27d393578e746ee3c37fd968a82a849bd4b8ff21d35ca6eb4`

The consumed payload was written to:

- `target/data-market-ui-audit-2026-03-21/consumed-dataset-from-prompt/payload`

The consumed files matched the original example dataset byte for byte.

## Fresh Repo-Owned Full Regression Run

After the prompt-driven proof, I reran the standard paid headless harness:

```bash
OPENAGENTS_HEADLESS_DATA_MARKET_E2E_RUN_DIR=target/headless-data-market-e2e-2026-03-21-audit \
OPENAGENTS_HEADLESS_DATA_MARKET_BUYER_PREFUND_SATS=100 \
scripts/autopilot/headless-data-market-e2e.sh
```

That run also completed successfully.

Important output:

- `target/headless-data-market-e2e-2026-03-21-audit/summary.json`

Key final IDs from that regression run:

- asset id:
  `data_asset.npub1kpavryheau45787fzxzlq9w83dled7yce8ssysmqle4wf0rhq79swe28jd.conversation_bundle.Headless_Dummy_Dataset.sha256_c0e0cf661545f117bd4e0611531b758f89e09d3c62fc3c8aaa4eebe16114299f`
- grant id:
  `access_grant.npub1kpavryheau45787fzxzlq9w83dled7yce8ssysmqle4wf0rhq79swe28jd.data_asset.npub1kpavryheau45787fzxzlq9w83dled7yce8ssysmqle4wf0rhq79swe28jd.conversation_bundle.Headless_Dummy_Dataset.sha256_c0e0cf661545f117bd4e0611531b758f89e09d3c62fc3c8aaa4eebe16114299f.targeted_request.npub1vr6u40npvee89pav2mxn5lgx3ecrck95p82sjzwjyswcmkh7j3hqztj6v6`
- request id:
  `6153fbff73317cddcad157ec0da6663274de9508f920afa8a86afb00dea531ca`
- seller payment pointer:
  `019d0f32-c1b2-70ec-a014-3944d57c6234`
- result event id:
  `db5e7496779c63d276819e5216a8dddeb951411f1fb57efe4d1944f627a5795e`

That regression run also delivered and consumed the payload successfully.

## Validation

Focused control-surface tests passed:

- `cargo test -p autopilot-desktop desktop_control_request_routes_align_with_ui_owned_actions -- --nocapture`
- `cargo test -p autopilot-desktop desktop_control_data_market_seller_prompt_payload_preserves_prompt -- --nocapture`
- `cargo test -p autopilot-desktop data_market_commands_map_to_control_requests -- --nocapture`

Full paid regression harness passed:

- `OPENAGENTS_HEADLESS_DATA_MARKET_E2E_RUN_DIR=target/headless-data-market-e2e-2026-03-21-audit OPENAGENTS_HEADLESS_DATA_MARKET_BUYER_PREFUND_SATS=100 scripts/autopilot/headless-data-market-e2e.sh`

## Rough Edge Observed

I saw one non-blocking UX rough edge during the prompt-driven seller pass:

- if the seller lane attempts grant preview before the asset has been published,
  seller status can briefly surface the expected
  `Publish an asset first. Grant creation requires a canonical DataAsset identity.`
  message while the asset side of the draft is otherwise healthy

That did not block the real flow and did not affect the final paid proof. The
actual required product order is still correct:

1. preview and publish asset
2. then preview and publish grant

## Conclusion

The seller flow we are telling users to use is now proven in the shape we claim:

- a one-sentence `Data Seller` prompt can prepare a real saleable draft
- the dataset can be published for sale locally with local `nexus-control`
- a buyer can buy it
- the seller can deliver it
- the buyer can consume it successfully

That is ready to present as the current local MVP path.
