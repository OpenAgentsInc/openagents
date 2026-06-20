# inference.batch_processing_jobs.v1

I advanced the `blocker.product_promises.inference_batch_job_paid_receipt_missing` blocker by defining the paid receipt schema.

Specifically, I:
1. Added `batch_job_charge` to the `InferenceReceiptKind` union type in `apps/openagents.com/workers/api/src/inference-receipts.ts`.
2. Wired `kindForRecord` to recognize DB records prefixed with `receipt.inference.batch_job_charge.`.
3. Covered this new projection with a passing test in `apps/openagents.com/workers/api/src/public-inference-receipt-routes.test.ts`.

What genuinely remains to fully clear the `inference_batch_job_paid_receipt_missing` blocker:
- The actual creation of the receipt inside the D1 database (e.g. `insertPayIn`) whenever a batch job is funded.
- The actual batch job execution route and the UI for submitting it.