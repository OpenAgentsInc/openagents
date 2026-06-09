import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsCloudflareContainerCloseoutReceipt,
  OpenAgentsCloudflareContainerCloseoutMissingScrubReceipt,
  OpenAgentsCloudflareContainerCloseoutProjection,
  OpenAgentsCloudflareContainerCloseoutUnsafeMaterial,
  openAgentsCloudflareContainerArtifactManifestFromCloseout,
  openAgentsCloudflareContainerGatewayCallbackFromCloseout,
  projectOpenAgentsCloudflareContainerCloseoutReceipt,
  validateOpenAgentsCloudflareContainerCloseoutReceipt,
} from './cloudflare-container-closeout-receipts'

const receipt = (
  overrides: Partial<OpenAgentsCloudflareContainerCloseoutReceipt> = {},
): OpenAgentsCloudflareContainerCloseoutReceipt => ({
  artifactCloseout: {
    buildLogRefs: ['build_log.container.redacted_summary'],
    diffRefs: ['diff.container.generated_site'],
    generatedFileRefs: ['file.container.index_html'],
    publicArtifactRefs: ['artifact.site.preview_url'],
    redactionReportRefs: ['redaction.container.clean'],
    screenshotRefs: ['screenshot.container.preview_desktop'],
    validationResultRefs: ['validation.container.build_passed'],
  },
  artifactManifestRef: 'manifest.container.closeout.1',
  backendKind: 'cloudflare_container',
  callbackRef: 'callback.runner.gateway.redacted_ref',
  closeoutReceiptRefs: ['receipt.container.closeout.completed'],
  credentialScrubReceiptRefs: ['receipt.container.scrub.credentials'],
  eventRefs: ['event.container.completed'],
  externalRunRef: 'cloudflare_container.run.1',
  operatorDiagnosticRefs: ['diagnostic.container.closeout.ready'],
  phase: 'completed',
  providerAccountScrubReceiptRefs: [
    'receipt.container.scrub.provider_account',
  ],
  publicSummaryRef: 'summary.container.completed',
  runRef: 'run.container.1',
  runnerId: 'runner.cloudflare_container.preview',
  statusCaveatRefs: ['caveat.container.preview_only'],
  ...overrides,
})

describe('Cloudflare Container callback and artifact closeout receipts', () => {
  test('validates terminal closeout only when credential and provider-account scrub refs exist', () => {
    expect(validateOpenAgentsCloudflareContainerCloseoutReceipt(receipt()))
      .toEqual(receipt())
    expect(
      validateOpenAgentsCloudflareContainerCloseoutReceipt(
        receipt({ credentialScrubReceiptRefs: [] }),
      ),
    ).toBeInstanceOf(OpenAgentsCloudflareContainerCloseoutMissingScrubReceipt)
    expect(
      validateOpenAgentsCloudflareContainerCloseoutReceipt(
        receipt({ phase: 'artifact', credentialScrubReceiptRefs: [] }),
      ),
    ).toMatchObject({ phase: 'artifact' })
  })

  test('denies raw logs, source archives, auth material, callback-token values, wallet secrets, and customer PII', () => {
    const unsafe = receipt({
      artifactCloseout: {
        ...receipt().artifactCloseout,
        buildLogRefs: ['raw_run_log.full_text'],
        generatedFileRefs: ['source_archive.raw_private_bundle'],
        redactionReportRefs: ['wallet_secret'],
      },
      callbackRef: 'callback_token_raw_value',
      operatorDiagnosticRefs: ['ben@example.com'],
    })

    expect(validateOpenAgentsCloudflareContainerCloseoutReceipt(unsafe))
      .toBeInstanceOf(OpenAgentsCloudflareContainerCloseoutUnsafeMaterial)
  })

  test('projects public, customer, team, and operator surfaces with increasing safe detail', () => {
    const publicProjection = projectOpenAgentsCloudflareContainerCloseoutReceipt(
      receipt(),
      'public',
    )
    const customerProjection =
      projectOpenAgentsCloudflareContainerCloseoutReceipt(receipt(), 'customer')
    const teamProjection = projectOpenAgentsCloudflareContainerCloseoutReceipt(
      receipt(),
      'team',
    )
    const operatorProjection =
      projectOpenAgentsCloudflareContainerCloseoutReceipt(receipt(), 'operator')

    expect(S.decodeUnknownSync(OpenAgentsCloudflareContainerCloseoutProjection)(
      operatorProjection,
    )).toEqual(operatorProjection)
    expect(publicProjection).toMatchObject({
      audience: 'public',
      buildLogRefs: [],
      diffRefs: [],
      eventRefs: [],
      generatedFileRefs: [],
      operatorDiagnosticRefs: [],
      phase: 'completed',
      publicArtifactRefs: ['artifact.site.preview_url'],
      scrubReceiptRefs: [],
      statusCaveatRefs: ['caveat.container.preview_only'],
    })
    expect(customerProjection).toMatchObject({
      audience: 'customer',
      buildLogRefs: [],
      diffRefs: ['diff.container.generated_site'],
      generatedFileRefs: ['file.container.index_html'],
      redactionReportRefs: ['redaction.container.clean'],
      screenshotRefs: ['screenshot.container.preview_desktop'],
      validationResultRefs: ['validation.container.build_passed'],
    })
    expect(teamProjection).toMatchObject({
      artifactManifestRef: 'manifest.container.closeout.1',
      audience: 'team',
      buildLogRefs: ['build_log.container.redacted_summary'],
      eventRefs: ['event.container.completed'],
      scrubReceiptRefs: [
        'receipt.container.scrub.credentials',
        'receipt.container.scrub.provider_account',
      ],
    })
    expect(operatorProjection).toMatchObject({
      audience: 'operator',
      callbackRef: 'callback.runner.gateway.redacted_ref',
      externalRunRef: 'cloudflare_container.run.1',
      operatorDiagnosticRefs: ['diagnostic.container.closeout.ready'],
    })
    expect(JSON.stringify(publicProjection)).not.toContain('callback')
    expect(JSON.stringify(customerProjection)).not.toContain('build_log')
  })

  test('integrates with gateway lifecycle callback and artifact manifest shapes', () => {
    expect(openAgentsCloudflareContainerGatewayCallbackFromCloseout(receipt()))
      .toEqual({
        artifactManifestRef: 'manifest.container.closeout.1',
        backendKind: 'cloudflare_container',
        callbackRef: 'callback.runner.gateway.redacted_ref',
        dispatchStatus: 'completed',
        eventRefs: ['event.container.completed'],
        externalRunRef: 'cloudflare_container.run.1',
        receiptRefs: ['receipt.container.closeout.completed'],
        runRef: 'run.container.1',
        runnerId: 'runner.cloudflare_container.preview',
      })
    expect(
      openAgentsCloudflareContainerGatewayCallbackFromCloseout(
        receipt({ phase: 'timed_out' }),
      ).dispatchStatus,
    ).toBe('failed')
    expect(openAgentsCloudflareContainerArtifactManifestFromCloseout(receipt()))
      .toEqual({
        artifactRefs: [
          'file.container.index_html',
          'diff.container.generated_site',
          'screenshot.container.preview_desktop',
          'build_log.container.redacted_summary',
          'validation.container.build_passed',
          'redaction.container.clean',
        ],
        digestRef: 'digest.manifest.container.closeout.1',
        manifestRef: 'manifest.container.closeout.1',
        publicArtifactRefs: ['artifact.site.preview_url'],
        receiptRefs: ['receipt.container.closeout.completed'],
      })
  })
})
