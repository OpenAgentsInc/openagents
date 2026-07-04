#!/usr/bin/env bun
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'

import {
  REACTOR_OPENAGENTS_DOGFOOD_AIRGAP_BUNDLE,
  REACTOR_OPENAGENTS_DOGFOOD_INSTALL_OPS_RECEIPT,
  REACTOR_OPENAGENTS_DOGFOOD_LOCAL_METERING_RECEIPT_SEED,
  REACTOR_OPENAGENTS_DOGFOOD_MODEL_INSTALL_RECEIPT,
  REACTOR_OPENAGENTS_DOGFOOD_QWEN_REFUSED_BUNDLE,
  REACTOR_OPENAGENTS_DOGFOOD_REFUSED_INSTALL_OPS_RECEIPT,
  REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED,
  REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT,
} from '../src/index'

const workDir = mkdtempSync(resolve(tmpdir(), 'reactor-dogfood-smoke-'))
const receiptsDir = resolve(workDir, 'receipts')

mkdirSync(receiptsDir)

const receipts = {
  dogfoodRun: REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT,
  freshInstall: REACTOR_OPENAGENTS_DOGFOOD_INSTALL_OPS_RECEIPT,
  freshModelInstall: REACTOR_OPENAGENTS_DOGFOOD_MODEL_INSTALL_RECEIPT,
  gptOssBundle: REACTOR_OPENAGENTS_DOGFOOD_AIRGAP_BUNDLE,
  localMetering: REACTOR_OPENAGENTS_DOGFOOD_LOCAL_METERING_RECEIPT_SEED,
  qwenRefusedBundle: REACTOR_OPENAGENTS_DOGFOOD_QWEN_REFUSED_BUNDLE,
  qwenRefusedInstall: REACTOR_OPENAGENTS_DOGFOOD_REFUSED_INSTALL_OPS_RECEIPT,
  routeDecisions: REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED,
}

for (const [name, receipt] of Object.entries(receipts)) {
  writeFileSync(resolve(receiptsDir, `${name}.json`), JSON.stringify(receipt, null, 2))
}

console.log(
  JSON.stringify(
    {
      ok: REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT.status === 'completed',
      dogfoodReceiptRef: REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT.receiptRef,
      exactMeteringReceiptRefs:
        REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT.localMeteringReceiptRefs,
      refusedNonconformingInstallOpsReceiptRef:
        REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT
          .refusedNonconformingInstallOpsReceiptRef,
      refusedNonconformingModelRef:
        REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT.refusedNonconformingModelRef,
      totalMeasuredTokens:
        REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT.totalMeasuredTokens,
      workDir,
    },
    null,
    2,
  ),
)
