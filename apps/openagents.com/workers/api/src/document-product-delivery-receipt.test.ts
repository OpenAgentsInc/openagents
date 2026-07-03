import { describe, expect, test } from 'vitest'

import {
  DOCUMENT_PRODUCT_DELIVERY_RECEIPT_DOC_VERSION,
  DocumentProductDeliveryReceiptInvariantError,
  type DocumentProductDeliveryInput,
  buildDocumentProductDeliveryReceipt,
  decodeDocumentProductDeliveryReceiptDocument,
  serializeDocumentProductDeliveryReceiptDocument,
  toDocumentProductDeliveryReceiptDocument,
  verifyDereferencedDocumentProductReceipt,
  verifyDocumentProductDelivery,
} from './document-product-delivery-receipt'

const baseInput = (
  overrides: Partial<DocumentProductDeliveryInput> = {},
): DocumentProductDeliveryInput => ({
  productKind: 'formation_document',
  workItemRef: 'work_item.document_product.fixture',
  intakeSpecRef: null,
  customerTemplateRefs: [],
  customerCorpusRefs: [],
  generatedDraftRefs: [],
  groundingCitationRefs: [],
  redactionReportRef: null,
  reviewDecision: {
    accepted: false,
    reviewerRole: 'practitioner',
    reviewReceiptRef: null,
  },
  deliveredDocumentRefs: [],
  deliveryReceiptRef: null,
  freshnessTimestamp: '2026-07-02T22:30:00.000Z',
  publicSourceRefs: [
    'docs/fable/ROADMAP_BIZ.md#bf-4--fulfill-the-deliverable-engine',
  ],
  ...overrides,
})

const deliveredInput = (): DocumentProductDeliveryInput =>
  baseInput({
    intakeSpecRef: 'intake.public.document_product.fixture.v1',
    customerTemplateRefs: ['template.public.customer_style.fixture.v1'],
    customerCorpusRefs: ['corpus.public.customer_facts.fixture.v1'],
    generatedDraftRefs: ['draft.public.document_product.fixture.v1'],
    groundingCitationRefs: [
      'citation.public.customer_fact.fixture.1',
      'citation.public.customer_template.fixture.1',
    ],
    redactionReportRef: 'redaction.public.document_product.fixture.pass.v1',
    reviewDecision: {
      accepted: true,
      reviewerRole: 'practitioner',
      reviewReceiptRef: 'review.public.document_product.fixture.accepted.v1',
    },
    deliveredDocumentRefs: ['document.public.delivered.fixture.v1'],
    deliveryReceiptRef: 'receipt.public.document_product.fixture.delivered.v1',
  })

describe('document-product delivery receipt', () => {
  test('empty pipeline is blocked and not delivered', () => {
    const receipt = buildDocumentProductDeliveryReceipt(baseInput())

    expect(receipt.pipelineStage).toBe('blocked')
    expect(receipt.evidenceState).toBe('not_yet_evidenced')
    expect(receipt.noAutoPublish).toBe(true)
    expect(receipt.noAutoSend).toBe(true)
    expect(receipt.noLegalAdvice).toBe(true)
    expect(receipt.outstandingAuthorityBlockers).toHaveLength(6)
    expect(verifyDocumentProductDelivery(receipt)).toContain(
      'typed intake spec not receipted',
    )
  })

  test('grounded draft waits for professional review and delivery receipt', () => {
    const receipt = buildDocumentProductDeliveryReceipt(
      baseInput({
        intakeSpecRef: 'intake.public.document_product.fixture.v1',
        customerTemplateRefs: ['template.public.customer_style.fixture.v1'],
        generatedDraftRefs: ['draft.public.document_product.fixture.v1'],
        groundingCitationRefs: ['citation.public.customer_template.fixture.1'],
        redactionReportRef: 'redaction.public.document_product.fixture.pass.v1',
      }),
    )

    expect(receipt.pipelineStage).toBe('generated_for_review')
    expect(receipt.evidenceState).toBe('generated_grounded_draft')
    expect(verifyDocumentProductDelivery(receipt)).toContain(
      'professional-review gate not accepted',
    )
  })

  test('complete intake -> grounded generation -> review -> delivery verifies', () => {
    const receipt = buildDocumentProductDeliveryReceipt(deliveredInput())

    expect(receipt.pipelineStage).toBe('delivered')
    expect(receipt.evidenceState).toBe('delivered')
    expect(receipt.outstandingAuthorityBlockers).toEqual([])
    expect(verifyDocumentProductDelivery(receipt)).toEqual([])
  })

  test('rejects delivered document refs while gates are blocked', () => {
    expect(() =>
      buildDocumentProductDeliveryReceipt(
        baseInput({
          intakeSpecRef: 'intake.public.document_product.fixture.v1',
          deliveredDocumentRefs: ['document.public.delivered.fixture.v1'],
        }),
      ),
    ).toThrow(DocumentProductDeliveryReceiptInvariantError)
  })

  test('rejects accepted professional review without a review receipt ref', () => {
    expect(() =>
      buildDocumentProductDeliveryReceipt(
        baseInput({
          reviewDecision: {
            accepted: true,
            reviewerRole: 'practitioner',
            reviewReceiptRef: null,
          },
        }),
      ),
    ).toThrow(DocumentProductDeliveryReceiptInvariantError)
  })

  test('rejects customer acceptance as the professional review gate', () => {
    expect(() =>
      buildDocumentProductDeliveryReceipt(
        baseInput({
          reviewDecision: {
            accepted: true,
            reviewerRole: 'customer',
            reviewReceiptRef: 'review.public.document_product.customer.v1',
          },
        }),
      ),
    ).toThrow(DocumentProductDeliveryReceiptInvariantError)
  })

  test('rejects private or raw customer refs at construction', () => {
    expect(() =>
      buildDocumentProductDeliveryReceipt(
        baseInput({
          customerCorpusRefs: ['raw_customer_document.private.fixture'],
        }),
      ),
    ).toThrow(DocumentProductDeliveryReceiptInvariantError)
  })
})

describe('document-product delivery receipt — dereferenceable document', () => {
  test('serialize is deterministic and round-trips through decode', () => {
    const document = toDocumentProductDeliveryReceiptDocument(
      buildDocumentProductDeliveryReceipt(deliveredInput()),
    )

    const body = serializeDocumentProductDeliveryReceiptDocument(document)
    expect(body).toBe(serializeDocumentProductDeliveryReceiptDocument(document))

    const decoded = decodeDocumentProductDeliveryReceiptDocument(body)
    expect(decoded.docVersion).toBe(
      DOCUMENT_PRODUCT_DELIVERY_RECEIPT_DOC_VERSION,
    )
    expect(decoded).toEqual(document)
  })

  test('a dereferenced delivered document product verifies clean', () => {
    const body = serializeDocumentProductDeliveryReceiptDocument(
      toDocumentProductDeliveryReceiptDocument(
        buildDocumentProductDeliveryReceipt(deliveredInput()),
      ),
    )

    expect(verifyDereferencedDocumentProductReceipt(body)).toEqual([])
  })

  test('a dereferenced grounded draft does not verify as delivered', () => {
    const body = serializeDocumentProductDeliveryReceiptDocument(
      toDocumentProductDeliveryReceiptDocument(
        buildDocumentProductDeliveryReceipt(
          baseInput({
            intakeSpecRef: 'intake.public.document_product.fixture.v1',
            customerTemplateRefs: ['template.public.customer_style.fixture.v1'],
            generatedDraftRefs: ['draft.public.document_product.fixture.v1'],
            groundingCitationRefs: [
              'citation.public.customer_template.fixture.1',
            ],
            redactionReportRef:
              'redaction.public.document_product.fixture.pass.v1',
          }),
        ),
      ),
    )

    const reasons = verifyDereferencedDocumentProductReceipt(body)
    expect(reasons).toContain(
      'pipeline stage is generated_for_review, not delivered',
    )
  })

  test('wrong-version body is rejected at decode', () => {
    const body = serializeDocumentProductDeliveryReceiptDocument(
      toDocumentProductDeliveryReceiptDocument(
        buildDocumentProductDeliveryReceipt(deliveredInput()),
      ),
    )
    const tampered = body.replace(
      DOCUMENT_PRODUCT_DELIVERY_RECEIPT_DOC_VERSION,
      'document.product.delivery_receipt.v999',
    )

    const reasons = verifyDereferencedDocumentProductReceipt(tampered)
    expect(reasons.length).toBeGreaterThan(0)
    expect(reasons[0]).toContain('receipt document failed to decode')
  })

  test('a body that strips delivered refs cannot pass as delivered', () => {
    const body = serializeDocumentProductDeliveryReceiptDocument(
      toDocumentProductDeliveryReceiptDocument(
        buildDocumentProductDeliveryReceipt(deliveredInput()),
      ),
    )
    const tampered = body.replace('"document.public.delivered.fixture.v1"', '')

    const reasons = verifyDereferencedDocumentProductReceipt(tampered)
    expect(reasons).toContain('no delivered document refs')
  })
})
