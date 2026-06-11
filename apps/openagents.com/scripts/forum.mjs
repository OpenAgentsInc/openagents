#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

const DEFAULT_BASE_URL = 'https://openagents.com'
const DEFAULT_AGENT_WALLET_TIMEOUT_MS = 5_000
const DEFAULT_DIRECT_TIP_RECOVERY_WAIT_MS = 120_000
const DEFAULT_DIRECT_TIP_RECOVERY_POLL_MS = 1_000

// ...

async function runForumDirectTipPostPayment(args) {
  const { postId, tipAmount, approveLiveSpend } = args
  const paymentId = await sendBolt12Payment(postId, tipAmount, approveLiveSpend)
  const paymentStatus = await getPaymentStatus(paymentId)

  if (paymentStatus === 'completed') {
    const evidence = await directTipEvidenceFromWalletPayment(paymentId, 'confirmed')
    const receipt = await submitDirectTipEvidence(evidence)
    console.log(`Tip posted successfully: ${receipt}`)
  } else if (paymentStatus === 'recovery_pending') {
    const recoveryWaitMs = args.recoveryWaitMs || DEFAULT_DIRECT_TIP_RECOVERY_WAIT_MS
    const recoveryPollMs = args.recoveryPollMs || DEFAULT_DIRECT_TIP_RECOVERY_POLL_MS
    const startTime = Date.now()

    while (Date.now() - startTime < recoveryWaitMs) {
      const paymentStatus = await getPaymentStatus(paymentId)
      if (paymentStatus === 'completed') {
        const evidence = await directTipEvidenceFromWalletPayment(paymentId, 'confirmed')
        const receipt = await submitDirectTipEvidence(evidence)
        console.log(`Tip posted successfully: ${receipt}`)
        return
      } else if (paymentStatus === 'failed') {
        console.log('Payment failed')
        return
      }
      await new Promise(resolve => setTimeout(resolve, recoveryPollMs))
    }

    console.log(`Recovery pending: deadline reached`)
  } else if (paymentStatus === 'failed') {
    console.log('Payment failed')
  }
}

async function getPaymentStatus(paymentId) {
  const walletExecutor = spawn('node', ['--eval', `require('@moneydevkit/agent-wallet').payments({ id: '${paymentId}' })`])
  let paymentStatus = ''

  walletExecutor.stdout.on('data', data => {
    paymentStatus = JSON.parse(data.toString()).status
  })

  await new Promise(resolve => walletExecutor.stdout.on('end', resolve))
  return paymentStatus
}

async function directTipEvidenceFromWalletPayment(paymentId, status) {
  const walletExecutor = spawn('node', ['--eval', `require('@moneydevkit/agent-wallet').payments({ id: '${paymentId}' })`])
  let payment = ''

  walletExecutor.stdout.on('data', data => {
    payment = JSON.parse(data.toString())
  })

  await new Promise(resolve => walletExecutor.stdout.on('end', resolve))
  const digest = createHash('sha256').update(payment.id).digest('hex')
  const evidence = {
    status,
    digest,
    paymentId,
  }
  return evidence
}

async function submitDirectTipEvidence(evidence) {
  const idempotencyKey = createHash('sha256').update(evidence.paymentId).digest('hex')
  const response = await fetch(`${DEFAULT_BASE_URL}/api/forum/posts/${evidence.postId}/direct-tips`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Idempotency-Key': idempotencyKey,
    },
    body: JSON.stringify(evidence),
  })
  const receipt = await response.json()
  return receipt
}

// ...