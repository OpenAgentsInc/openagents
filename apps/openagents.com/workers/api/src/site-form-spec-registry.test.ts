import { describe, expect, test } from 'vitest'

import {
  resolveSiteFormSpec,
  resolveSiteFormSpecs,
} from './site-form-spec-registry'

const leadFormSpec = {
  id: 'lead.newsletter',
  listId: 'list.newsletter',
  fields: [
    { name: 'email', kind: 'email' as const, required: true },
    { name: 'name', kind: 'text' as const },
  ],
}

const metadataObject = {
  title: 'Acme launch site',
  formSpecs: {
    'lead.newsletter': leadFormSpec,
    'lead.contact': {
      id: 'lead.contact',
      listId: 'list.contact',
      fields: [{ name: 'email', kind: 'email' as const }],
    },
  },
}

describe('site form-spec registry', () => {
  test('resolves a typed form spec by id from a metadata object', () => {
    const spec = resolveSiteFormSpec(metadataObject, 'lead.newsletter')
    expect(spec).toEqual(leadFormSpec)
  })

  test('resolves a form spec when metadata is a JSON string', () => {
    const spec = resolveSiteFormSpec(
      JSON.stringify(metadataObject),
      'lead.contact',
    )
    expect(spec?.listId).toBe('list.contact')
  })

  test('returns every published spec from the registry', () => {
    const specs = resolveSiteFormSpecs(metadataObject)
    expect([...specs.keys()].sort()).toEqual(['lead.contact', 'lead.newsletter'])
  })

  test('returns undefined for an unknown form id (route renders 404)', () => {
    expect(resolveSiteFormSpec(metadataObject, 'lead.missing')).toBeUndefined()
  })

  test('drops a misfiled spec whose own id disagrees with its key', () => {
    const mismatched = {
      formSpecs: {
        'lead.alias': { ...leadFormSpec, id: 'lead.real' },
      },
    }
    expect(resolveSiteFormSpec(mismatched, 'lead.alias')).toBeUndefined()
    expect(resolveSiteFormSpecs(mismatched).size).toBe(0)
  })

  test('degrades to an empty registry for malformed metadata (no throw)', () => {
    expect(resolveSiteFormSpecs('not json {').size).toBe(0)
    expect(resolveSiteFormSpecs(undefined).size).toBe(0)
    expect(resolveSiteFormSpecs(42).size).toBe(0)
    expect(resolveSiteFormSpecs({ formSpecs: { x: { id: 'x' } } }).size).toBe(0)
  })

  test('ignores unrelated metadata keys and absent formSpecs', () => {
    expect(resolveSiteFormSpecs({ title: 'no forms here' }).size).toBe(0)
  })
})
