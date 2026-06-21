# inference.batch_processing_jobs.v1

This launch builds the paid receipt surface for `inference.batch_processing_jobs.v1`, clearing `blocker.product_promises.inference_batch_job_paid_receipt_missing`.

The single-request gateway exists and free allowance is synergized, but batch-job capabilities required a way to meter the payload correctly and provide a receipt for the processed items upon closeout.

## Work Shipped

1. **Job Persistence**: Wired `makeD1BatchJobStore().insertBatchJob(...)` into the POST `/v1/inference/batches` submit route. The payload items are already metered via `settleBatchJobCharge`, and now the initial pending state plus `chargeReceiptRef` and `datasetSize` are persisted to `inference_batch_jobs`.
2. **Closeout Receipt Schema**: Utilized the existing `BatchJobCloseoutReceiptSchema` which details items processed, charge reference, success/failure counts, and R2 result keys.
3. **Public Receipt Read Route**: Implemented `handleBatchJobReceiptRead` at `GET /api/public/inference/batch-job-receipts/:receiptRef`. It reads the closed job from `inference_batch_jobs` (denying access to pending jobs), fetches the actual `totalCostMsat` from the `pay_ins` ledger using the job's `chargeReceiptRef`, constructs the `BatchJobCloseoutReceipt`, and emits it under the strict public projection staleness contract.
4. **Architecture Integration**: The new route has been registered in the `staleness_declared` budget (`check-zero-debt-architecture.mjs`) and is itemized within `INVARIANTS.md`. Tests for `handleBatchJobReceiptRead` and the existing DB logic are implemented in `batch-job-routes.test.ts`.
5. **Status Route**: Implemented `handleBatchJobStatusRead` at `GET /v1/inference/batches/:jobId` to permit authenticated users to read the status of their pending or processing jobs, fully closing the loop on the user-facing job monitoring surface.

## Remaining Work

The promise remains `planned`. While the API can now accept jobs and issue proper receipts, `blocker.product_promises.inference_batch_job_surface_unbuilt` remains: there is still no background job processing pipeline that actually downloads, runs the Sybil-gated Sybil models, processes data through R2, and ultimately transitions the job status to `completed` while publishing the final `resultsR2Key`.

*Update 2026-06-20*: Updated `product-promises.ts` registry pass 2026-06-20.57 to formally clear `blocker.product_promises.inference_batch_job_paid_receipt_missing` since the code implementation is completed.
