import { describe, expect, it } from 'bun:test'

import { AiElements } from '../src/index'
import {
  aiElementBaseTag,
  aiElementModuleCount,
  aiElementPorts,
  aiElementPrimitiveCount,
} from '../src/ai-elements'

// The AI Elements catalog spec for this package. Mirrors the Maud curation map
// (`autopilot4-deprecated/src/ui_components/ai_elements.rs`) for the priority
// subset issue #5083 ships: module ids in priority order plus each module's
// primitive count. This is the spec the family is checked against, in the same
// fail-closed style as the Tailwind UI v4 family-coverage test.
const expectedCatalog = [
  { moduleId: 'prompt-input', primitives: 7 },
  { moduleId: 'message', primitives: 4 },
  { moduleId: 'response', primitives: 3 },
  { moduleId: 'code-block', primitives: 5 },
  { moduleId: 'task', primitives: 5 },
  { moduleId: 'sources', primitives: 4 },
  { moduleId: 'tool', primitives: 5 },
  { moduleId: 'confirmation', primitives: 4 },
  { moduleId: 'reasoning', primitives: 3 },
  { moduleId: 'web-preview', primitives: 5 },
] as const

const expectedModuleCount = expectedCatalog.length
const expectedPrimitiveCount = expectedCatalog.reduce(
  (total, entry) => total + entry.primitives,
  0,
)

describe('AI Elements catalog coverage', () => {
  it('ships every priority module in order', () => {
    expect(aiElementPorts.map(port => port.moduleId)).toEqual(
      expectedCatalog.map(entry => entry.moduleId),
    )
  })

  it('tracks the expected module and primitive counts', () => {
    expect(aiElementModuleCount()).toBe(expectedModuleCount)
    expect(aiElementPrimitiveCount()).toBe(expectedPrimitiveCount)
  })

  it('records the curated primitive count per module', () => {
    for (const entry of expectedCatalog) {
      const port = aiElementPorts.find(p => p.moduleId === entry.moduleId)

      expect(port).toBeDefined()
      expect(port?.primitives.length).toBe(entry.primitives)
    }
  })

  it('has no duplicate module ids and a non-empty primitive list per module', () => {
    const ids = aiElementPorts.map(port => port.moduleId)

    expect(new Set(ids).size).toBe(ids.length)

    for (const port of aiElementPorts) {
      expect(port.primitives.length).toBeGreaterThan(0)
      expect(new Set(port.primitives).size).toBe(port.primitives.length)
    }
  })

  it('emits a stable ai-elements base-contract tag', () => {
    expect(aiElementBaseTag('prompt-input', 'PromptInput')).toBe(
      'ai-elements:prompt-input/PromptInput',
    )
  })

  it('exposes the family through the package barrel namespace', () => {
    expect(typeof AiElements.promptInput).toBe('function')
    expect(typeof AiElements.message).toBe('function')
    expect(typeof AiElements.response).toBe('function')
    expect(typeof AiElements.codeBlock).toBe('function')
    expect(typeof AiElements.task).toBe('function')
    expect(typeof AiElements.sources).toBe('function')
    expect(typeof AiElements.tool).toBe('function')
    expect(typeof AiElements.confirmation).toBe('function')
    expect(typeof AiElements.reasoning).toBe('function')
    expect(typeof AiElements.webPreview).toBe('function')
    expect(AiElements.aiElementModuleCount()).toBe(expectedModuleCount)
  })
})
