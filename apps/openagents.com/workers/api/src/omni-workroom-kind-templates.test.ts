import { describe, expect, test } from 'vitest'

import {
  OMNI_WORKROOM_KIND_TEMPLATES,
  OmniWorkroomKindTemplateValidationError,
  getOmniWorkroomKindTemplate,
  validateOmniWorkroomKindEvidence,
  validateOmniWorkroomKindTemplate,
} from './omni-workroom-kind-templates'

describe('Omni workroom kind templates', () => {
  test('covers every supported workroom kind', () => {
    expect(Object.keys(OMNI_WORKROOM_KIND_TEMPLATES).sort()).toEqual([
      'coding',
      'crm',
      'document',
      'finance_ops',
      'investor_ops',
      'legal_review',
      'meeting',
      'project_ops',
      'site',
      'support',
    ])
  })

  test('defines a public-safe Sites launch and revision template', () => {
    const template = validateOmniWorkroomKindTemplate(
      getOmniWorkroomKindTemplate('site'),
    )

    expect(template.acceptedOutcomeWorkKind).toBe('site')
    expect(template.reviewPolicy).toBe('customer_review')
    expect(template.proofPolicy).toBe('public_safe_proof')
    expect(template.publicProjectionPolicy).toBe('public_safe_proof')
    expect(template.closeoutRequirements).toContain('deployment_live')
    expect(template.closeoutRequirements).toContain('email_sent')
    expect(template.requiredEvidence.map(item => item.entryKind)).toEqual([
      'deployment_url',
      'screenshot',
      'test_report',
      'email_receipt',
    ])

    expect(() =>
      validateOmniWorkroomKindEvidence('site', [
        'deployment_url',
        'screenshot',
        'test_report',
        'email_receipt',
      ]),
    ).not.toThrow()
  })

  test('defines coding as a customer-safe PR or patch workroom', () => {
    const template = validateOmniWorkroomKindTemplate(
      getOmniWorkroomKindTemplate('coding'),
    )

    expect(template.acceptedOutcomeWorkKind).toBe('coding')
    expect(template.reviewPolicy).toBe('customer_review')
    expect(template.publicProjectionPolicy).toBe('customer_safe_summary')
    expect(template.requiredArtifacts.map(item => item.artifactKind)).toEqual([
      'pull_request',
      'diff',
      'source_commit',
      'test_report',
    ])

    expect(() =>
      validateOmniWorkroomKindEvidence('coding', [
        'diff',
        'source_commit',
        'test_report',
      ]),
    ).not.toThrow()
  })

  test('keeps legal review private with dual review and redaction requirements', () => {
    const template = validateOmniWorkroomKindTemplate(
      getOmniWorkroomKindTemplate('legal_review'),
    )

    expect(template.acceptedOutcomeWorkKind).toBe('legal_sensitive')
    expect(template.privacyConstraint).toBe('legal_private')
    expect(template.publicProjectionPolicy).toBe('none')
    expect(template.proofPolicy).toBe('legal_sensitive_private')
    expect(template.reviewPolicy).toBe('dual_review')
    expect(template.closeoutRequirements).toContain('legal_review')
    expect(template.closeoutRequirements).toContain('redaction_passed')
  })

  test('rejects public projection templates with private required evidence', () => {
    expect(() =>
      validateOmniWorkroomKindTemplate({
        ...getOmniWorkroomKindTemplate('site'),
        requiredEvidence: [
          {
            entryKind: 'build_log',
            publicSafeAllowed: false,
            required: true,
          },
        ],
      }),
    ).toThrow(OmniWorkroomKindTemplateValidationError)
  })

  test('rejects legal-private templates with public projection', () => {
    expect(() =>
      validateOmniWorkroomKindTemplate({
        ...getOmniWorkroomKindTemplate('legal_review'),
        publicProjectionPolicy: 'public_safe_proof',
      }),
    ).toThrow(OmniWorkroomKindTemplateValidationError)
  })

  test('reports missing required evidence for a kind', () => {
    expect(() =>
      validateOmniWorkroomKindEvidence('site', ['deployment_url']),
    ).toThrow(OmniWorkroomKindTemplateValidationError)
  })
})
