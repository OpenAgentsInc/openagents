import { describe, expect, it } from 'bun:test'

import {
  type CodeToken,
  type CodeTokenKind,
  codeTokenClass,
  isHighlightableLanguage,
  tokenizeCode,
  tokenizeCodeLines,
} from '../src/ai-elements/code-highlight'

const join = (tokens: ReadonlyArray<CodeToken>): string =>
  tokens.map(token => token.text).join('')

const kinds = (tokens: ReadonlyArray<CodeToken>): ReadonlySet<CodeTokenKind> =>
  new Set(tokens.map(token => token.kind))

const textsOfKind = (
  tokens: ReadonlyArray<CodeToken>,
  kind: CodeTokenKind,
): ReadonlyArray<string> =>
  tokens.filter(token => token.kind === kind).map(token => token.text)

describe('tokenizeCode', () => {
  it('is byte-faithful: concatenated token text equals the input', () => {
    const samples: ReadonlyArray<readonly [string, string]> = [
      ['const x = `a${y}b` // note\n', 'typescript'],
      ['def f(n):\n    return n + 1  # add\n', 'python'],
      ['fn main() { let v = vec![1, 2]; }', 'rust'],
      ['func main() {\n\tx := 1\n}\n', 'go'],
      ['{ "a": 1, "b": [true, null] }', 'json'],
      ['echo "$HOME/bin" # path', 'bash'],
      ['', 'typescript'],
      ['plain text with no grammar', 'unknownlang'],
    ]
    for (const [code, language] of samples) {
      expect(join(tokenizeCode(code, language))).toBe(code)
    }
  })

  it('classifies TypeScript keywords, strings, comments, and numbers', () => {
    const tokens = tokenizeCode(
      'const total = 42 // count\nconst label = "hi"',
      'typescript',
    )
    expect(textsOfKind(tokens, 'keyword')).toContain('const')
    expect(textsOfKind(tokens, 'number')).toContain('42')
    expect(textsOfKind(tokens, 'string')).toContain('"hi"')
    expect(textsOfKind(tokens, 'comment').some(t => t.includes('count'))).toBe(
      true,
    )
  })

  it('marks JSON object keys as property and literals as constant', () => {
    const tokens = tokenizeCode('{ "id": 7, "ok": true }', 'json')
    expect(textsOfKind(tokens, 'property')).toContain('"id"')
    expect(textsOfKind(tokens, 'constant')).toContain('true')
    expect(textsOfKind(tokens, 'number')).toContain('7')
  })

  it('detects call expressions as function tokens', () => {
    const tokens = tokenizeCode('greet(name)', 'typescript')
    expect(textsOfKind(tokens, 'function')).toContain('greet')
  })

  it('treats shell variables as property tokens', () => {
    const tokens = tokenizeCode('echo ${PATH}', 'bash')
    expect(textsOfKind(tokens, 'property')).toContain('${PATH}')
  })

  it('falls back to a single plain token for unknown languages', () => {
    const tokens = tokenizeCode('whatever this is', 'cobol')
    expect(tokens).toEqual([{ kind: 'plain', text: 'whatever this is' }])
    expect(kinds(tokens)).toEqual(new Set(['plain']))
  })
})

describe('tokenizeCodeLines', () => {
  it('splits into per-line rows without embedded newlines', () => {
    const lines = tokenizeCodeLines('a\nb\nc', 'unknown')
    expect(lines.length).toBe(3)
    for (const line of lines) {
      for (const token of line) {
        expect(token.text.includes('\n')).toBe(false)
      }
    }
    expect(join(lines.flat())).toBe('abc')
  })

  it('keeps a trailing empty row for a trailing newline', () => {
    const lines = tokenizeCodeLines('x\n', 'typescript')
    expect(lines.length).toBe(2)
    expect(lines[1]).toEqual([])
  })
})

describe('codeTokenClass', () => {
  it('returns a Tailwind color class for every kind', () => {
    const all: ReadonlyArray<CodeTokenKind> = [
      'plain', 'comment', 'keyword', 'string', 'number', 'constant',
      'function', 'type', 'property', 'operator', 'punctuation',
    ]
    for (const kind of all) {
      expect(codeTokenClass(kind)).toMatch(/text-\[#/)
    }
  })
})

describe('isHighlightableLanguage', () => {
  it('recognizes known language aliases and rejects unknowns', () => {
    expect(isHighlightableLanguage('ts')).toBe(true)
    expect(isHighlightableLanguage('Python')).toBe(true)
    expect(isHighlightableLanguage('rs')).toBe(true)
    expect(isHighlightableLanguage(undefined)).toBe(false)
    expect(isHighlightableLanguage('cobol')).toBe(false)
  })
})
