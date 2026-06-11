import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { test, expect } from '@jest/globals'

test('tip-post payment completes after timeout', async () => {
  const paymentId = 'payment-id'
  const postId = 'post-id'
  const tipAmount = 15
  const approveLiveSpend = true
  const recoveryWaitMs = 10000
  const recoveryPollMs = 1000

  const walletExecutor = jest.fn().mockImplementation(() => {
    return {
      stdout: {
        on: jest.fn((event, callback) => {
          if (event === 'data') {
            callback(JSON.stringify({ status: 'recovery_pending' }))
          } else if (event === 'end') {
            callback()
          }
        }),
      },
    }
  })

  const getPaymentStatus = jest.fn().mockImplementation(async () => {
    await new Promise(resolve => setTimeout(resolve, recoveryPollMs))
    return 'completed'
  })

  const directTipEvidenceFromWalletPayment = jest.fn().mockImplementation(async () => {
    return {
      status: 'confirmed',
      digest: createHash('sha256').update(paymentId).digest('hex'),
      paymentId,
    }
  })

  const submitDirectTipEvidence = jest.fn().mockImplementation(async () => {
    return {
      receipt: 'receipt',
    }
  })

  const runForumDirectTipPostPayment = await import('../apps/openagents.com/scripts/forum.mjs')
  const result = await runForumDirectTipPostPayment.runForumDirectTipPostPayment({
    postId,
    tipAmount,
    approveLiveSpend,
    recoveryWaitMs,
    recoveryPollMs,
  })

  expect(result).toBeUndefined()
  expect(walletExecutor).toHaveBeenCalledTimes(1)
  expect(getPaymentStatus).toHaveBeenCalledTimes(2)
  expect(directTipEvidenceFromWalletPayment).toHaveBeenCalledTimes(1)
  expect(submitDirectTipEvidence).toHaveBeenCalledTimes(1)
})

test('tip-post payment fails after timeout', async () => {
  const paymentId = 'payment-id'
  const postId = 'post-id'
  const tipAmount = 15
  const approveLiveSpend = true
  const recoveryWaitMs = 10000
  const recoveryPollMs = 1000

  const walletExecutor = jest.fn().mockImplementation(() => {
    return {
      stdout: {
        on: jest.fn((event, callback) => {
          if (event === 'data') {
            callback(JSON.stringify({ status: 'recovery_pending' }))
          } else if (event === 'end') {
            callback()
          }
        }),
      },
    }
  })

  const getPaymentStatus = jest.fn().mockImplementation(async () => {
    await new Promise(resolve => setTimeout(resolve, recoveryPollMs))
    return 'failed'
  })

  const directTipEvidenceFromWalletPayment = jest.fn().mockImplementation(async () => {
    return {
      status: 'confirmed',
      digest: createHash('sha256').update(paymentId).digest('hex'),
      paymentId,
    }
  })

  const submitDirectTipEvidence = jest.fn().mockImplementation(async () => {
    return {
      receipt: 'receipt',
    }
  })

  const runForumDirectTipPostPayment = await import('../apps/openagents.com/scripts/forum.mjs')
  const result = await runForumDirectTipPostPayment.runForumDirectTipPostPayment({
    postId,
    tipAmount,
    approveLiveSpend,
    recoveryWaitMs,
    recoveryPollMs,
  })

  expect(result).toBeUndefined()
  expect(walletExecutor).toHaveBeenCalledTimes(1)
  expect(getPaymentStatus).toHaveBeenCalledTimes(2)
  expect(directTipEvidenceFromWalletPayment).not.toHaveBeenCalled()
  expect(submitDirectTipEvidence).not.toHaveBeenCalled()
})

test('tip-post payment never reaches terminal state', async () => {
  const paymentId = 'payment-id'
  const postId = 'post-id'
  const tipAmount = 15
  const approveLiveSpend = true
  const recoveryWaitMs = 10000
  const recoveryPollMs = 1000

  const walletExecutor = jest.fn().mockImplementation(() => {
    return {
      stdout: {
        on: jest.fn((event, callback) => {
          if (event === 'data') {
            callback(JSON.stringify({ status: 'recovery_pending' }))
          } else if (event === 'end') {
            callback()
          }
        }),
      },
    }
  })

  const getPaymentStatus = jest.fn().mockImplementation(async () => {
    await new Promise(resolve => setTimeout(resolve, recoveryPollMs))
    return 'recovery_pending'
  })

  const directTipEvidenceFromWalletPayment = jest.fn().mockImplementation(async () => {
    return {
      status: 'confirmed',
      digest: createHash('sha256').update(paymentId).digest('hex'),
      paymentId,
    }
  })

  const submitDirectTipEvidence = jest.fn().mockImplementation(async () => {
    return {
      receipt: 'receipt',
    }
  })

  const runForumDirectTipPostPayment = await import('../apps/openagents.com/scripts/forum.mjs')
  const result = await runForumDirectTipPostPayment.runForumDirectTipPostPayment({
    postId,
    tipAmount,
    approveLiveSpend,
    recoveryWaitMs,
    recoveryPollMs,
  })

  expect(result).toBeUndefined()
  expect(walletExecutor).toHaveBeenCalledTimes(1)
  expect(getPaymentStatus).toHaveBeenCalledTimes(2)
  expect(directTipEvidenceFromWalletPayment).not.toHaveBeenCalled()
  expect(submitDirectTipEvidence).not.toHaveBeenCalled()
})