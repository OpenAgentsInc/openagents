// Dependency-free, server-safe syntax tokenizer for the AI Elements code block.
//
// Why hand-rolled rather than Shiki/Prism: the kit renders typed Foldkit nodes
// and bans raw `innerHTML` for untrusted content (user/agent code), so we cannot
// inject a highlighter's HTML string into the vdom. Instead we tokenize source
// into a flat list of `{ kind, text }` tokens that the component renders as
// typed `h.span` nodes — fully escaped, deterministic, and unit-testable, with
// no new runtime dependency.
//
// The token theme is the house Protoss palette (DESIGN.md): cool blue/cyan on a
// near-black sunken surface, white-ish identifiers, muted comments. No violet
// (house ban), no light variant.

export type CodeTokenKind =
  | 'plain'
  | 'comment'
  | 'keyword'
  | 'string'
  | 'number'
  | 'constant'
  | 'function'
  | 'type'
  | 'property'
  | 'operator'
  | 'punctuation'

export type CodeToken = {
  readonly kind: CodeTokenKind
  readonly text: string
}

// Tailwind arbitrary-color classes per kind. Kept in the inline-Tailwind-hex
// idiom used by the other ai-elements files (tool/web-preview/response), so no
// CSS file / token-migration test is involved.
const TOKEN_CLASS: Record<CodeTokenKind, string> = {
  plain: 'text-[#d7e2f0]',
  comment: 'text-[#6b7a90] italic',
  keyword: 'text-[#7aa2ff]',
  string: 'text-[#9ed1ff]',
  number: 'text-[#4fd0ff]',
  constant: 'text-[#8fb6ff]',
  function: 'text-[#cfe3ff]',
  type: 'text-[#67d4ff]',
  property: 'text-[#aecbff]',
  operator: 'text-[#9fb2c9]',
  punctuation: 'text-[#7e8a98]',
}

export const codeTokenClass = (kind: CodeTokenKind): string => TOKEN_CLASS[kind]

type Grammar = {
  readonly lineComments: ReadonlyArray<string>
  readonly blockComment: readonly [string, string] | null
  // String delimiters that may span lines (template/backtick, triple-quote).
  readonly multilineStrings: ReadonlyArray<string>
  // Single-line string delimiters.
  readonly strings: ReadonlyArray<string>
  readonly keywords: ReadonlySet<string>
  readonly constants: ReadonlySet<string>
  readonly types: ReadonlySet<string>
  // `$`-prefixed shell-style variables become `property` tokens.
  readonly shellVariables: boolean
  // JSON object keys (a string immediately before `:`) become `property`.
  readonly jsonKeys: boolean
}

const set = (...words: ReadonlyArray<string>): ReadonlySet<string> =>
  new Set(words)

const JS_KEYWORDS = set(
  'abstract', 'as', 'async', 'await', 'break', 'case', 'catch', 'class',
  'const', 'continue', 'debugger', 'declare', 'default', 'delete', 'do',
  'else', 'enum', 'export', 'extends', 'finally', 'for', 'from', 'function',
  'get', 'if', 'implements', 'import', 'in', 'infer', 'instanceof',
  'interface', 'keyof', 'let', 'namespace', 'new', 'of', 'private',
  'protected', 'public', 'readonly', 'return', 'satisfies', 'set', 'static',
  'switch', 'throw', 'try', 'type', 'typeof', 'var', 'void', 'while', 'with',
  'yield',
)
const JS_CONSTANTS = set(
  'true', 'false', 'null', 'undefined', 'NaN', 'Infinity', 'this', 'super',
)
const JS_TYPES = set(
  'string', 'number', 'boolean', 'object', 'unknown', 'any', 'never', 'bigint',
  'symbol', 'Promise', 'Array', 'Record', 'Readonly', 'Partial',
)

const PY_KEYWORDS = set(
  'and', 'as', 'assert', 'async', 'await', 'break', 'class', 'continue', 'def',
  'del', 'elif', 'else', 'except', 'finally', 'for', 'from', 'global', 'if',
  'import', 'in', 'is', 'lambda', 'nonlocal', 'not', 'or', 'pass', 'raise',
  'return', 'try', 'while', 'with', 'yield', 'match', 'case',
)
const PY_CONSTANTS = set('True', 'False', 'None', 'self', 'cls')

const RUST_KEYWORDS = set(
  'as', 'async', 'await', 'break', 'const', 'continue', 'crate', 'dyn', 'else',
  'enum', 'extern', 'fn', 'for', 'if', 'impl', 'in', 'let', 'loop', 'match',
  'mod', 'move', 'mut', 'pub', 'ref', 'return', 'static', 'struct', 'super',
  'trait', 'type', 'unsafe', 'use', 'where', 'while', 'dyn',
)
const RUST_CONSTANTS = set(
  'true', 'false', 'Some', 'None', 'Ok', 'Err', 'self', 'Self',
)

const GO_KEYWORDS = set(
  'break', 'case', 'chan', 'const', 'continue', 'default', 'defer', 'else',
  'fallthrough', 'for', 'func', 'go', 'goto', 'if', 'import', 'interface',
  'map', 'package', 'range', 'return', 'select', 'struct', 'switch', 'type',
  'var',
)
const GO_CONSTANTS = set('true', 'false', 'nil', 'iota')

const BASH_KEYWORDS = set(
  'if', 'then', 'else', 'elif', 'fi', 'for', 'while', 'until', 'do', 'done',
  'case', 'esac', 'function', 'in', 'select', 'return', 'export', 'local',
  'readonly', 'declare', 'unset', 'source', 'cd', 'echo', 'set',
)

const GRAMMARS: Record<string, Grammar> = {
  js: {
    lineComments: ['//'],
    blockComment: ['/*', '*/'],
    multilineStrings: ['`'],
    strings: ['"', "'"],
    keywords: JS_KEYWORDS,
    constants: JS_CONSTANTS,
    types: JS_TYPES,
    shellVariables: false,
    jsonKeys: false,
  },
  python: {
    lineComments: ['#'],
    blockComment: null,
    multilineStrings: ['"""', "'''"],
    strings: ['"', "'"],
    keywords: PY_KEYWORDS,
    constants: PY_CONSTANTS,
    types: set(),
    shellVariables: false,
    jsonKeys: false,
  },
  rust: {
    lineComments: ['//'],
    blockComment: ['/*', '*/'],
    multilineStrings: [],
    strings: ['"'],
    keywords: RUST_KEYWORDS,
    constants: RUST_CONSTANTS,
    types: set(),
    shellVariables: false,
    jsonKeys: false,
  },
  go: {
    lineComments: ['//'],
    blockComment: ['/*', '*/'],
    multilineStrings: ['`'],
    strings: ['"'],
    keywords: GO_KEYWORDS,
    constants: GO_CONSTANTS,
    types: set(),
    shellVariables: false,
    jsonKeys: false,
  },
  json: {
    lineComments: [],
    blockComment: null,
    multilineStrings: [],
    strings: ['"'],
    keywords: set(),
    constants: set('true', 'false', 'null'),
    types: set(),
    shellVariables: false,
    jsonKeys: true,
  },
  bash: {
    lineComments: ['#'],
    blockComment: null,
    multilineStrings: [],
    strings: ['"', "'"],
    keywords: BASH_KEYWORDS,
    constants: set('true', 'false'),
    types: set(),
    shellVariables: true,
    jsonKeys: false,
  },
}

const LANGUAGE_ALIASES: Record<string, keyof typeof GRAMMARS> = {
  ts: 'js', tsx: 'js', typescript: 'js', 'typescriptreact': 'js',
  js: 'js', jsx: 'js', javascript: 'js', 'javascriptreact': 'js',
  mjs: 'js', cjs: 'js', node: 'js',
  py: 'python', python: 'python', py3: 'python',
  rs: 'rust', rust: 'rust',
  go: 'go', golang: 'go',
  json: 'json', jsonc: 'json', json5: 'json',
  sh: 'bash', bash: 'bash', shell: 'bash', zsh: 'bash', console: 'bash',
  shellscript: 'bash',
}

const grammarFor = (language?: string): Grammar | null => {
  if (language === undefined) return null
  const key = LANGUAGE_ALIASES[language.trim().toLowerCase()]
  return key === undefined ? null : (GRAMMARS[key] ?? null)
}

/** Whether the tokenizer has a grammar for this language (else plain text). */
export const isHighlightableLanguage = (language?: string): boolean =>
  grammarFor(language) !== null

const isIdentifierStart = (ch: string): boolean =>
  /[A-Za-z_$]/.test(ch)
const isIdentifierPart = (ch: string): boolean =>
  /[A-Za-z0-9_$]/.test(ch)
const isDigit = (ch: string): boolean => /[0-9]/.test(ch)
const PUNCTUATION = new Set([...'{}()[];,.'])
const OPERATOR = new Set([...'+-*/%=<>!&|^~?:@'])

const startsWith = (
  source: string,
  index: number,
  token: string,
): boolean => source.startsWith(token, index)

const classifyIdentifier = (
  grammar: Grammar,
  name: string,
  followedByCall: boolean,
  precededByDot: boolean,
): CodeTokenKind => {
  if (precededByDot) {
    return followedByCall ? 'function' : 'property'
  }
  if (grammar.constants.has(name)) return 'constant'
  if (grammar.keywords.has(name)) return 'keyword'
  if (grammar.types.has(name)) return 'type'
  if (followedByCall) return 'function'
  // Capitalized identifier → treat as a type/class name (Rust/Go/TS idiom).
  if (/^[A-Z]/.test(name) && /[a-z]/.test(name)) return 'type'
  return 'plain'
}

const peekNonSpace = (source: string, from: number): string => {
  let i = from
  while (
    i < source.length &&
    (source.charAt(i) === ' ' || source.charAt(i) === '\t')
  ) {
    i += 1
  }
  // `charAt` returns '' past the end, matching the "no non-space char" case.
  return source.charAt(i)
}

// Tokenize source into a flat token list. Every character of `code` is preserved
// across the tokens (including whitespace and newlines), so the rendered body is
// byte-faithful to the input.
export const tokenizeCode = (
  code: string,
  language?: string,
): ReadonlyArray<CodeToken> => {
  const grammar = grammarFor(language)
  if (grammar === null) {
    return code.length === 0 ? [] : [{ kind: 'plain', text: code }]
  }

  const tokens: CodeToken[] = []
  let plain = ''
  const flush = (): void => {
    if (plain.length > 0) {
      tokens.push({ kind: 'plain', text: plain })
      plain = ''
    }
  }
  const push = (kind: CodeTokenKind, text: string): void => {
    if (text.length === 0) return
    flush()
    tokens.push({ kind, text })
  }

  const n = code.length
  let i = 0
  let lastSignificant = '' // last non-space token text, for `.property` detection

  while (i < n) {
    const ch = code.charAt(i)

    // Whitespace → accumulate as plain.
    if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
      plain += ch
      i += 1
      continue
    }

    // Line comments.
    let matchedComment = false
    for (const marker of grammar.lineComments) {
      if (startsWith(code, i, marker)) {
        let j = i
        while (j < n && code.charAt(j) !== '\n') j += 1
        push('comment', code.slice(i, j))
        i = j
        matchedComment = true
        break
      }
    }
    if (matchedComment) {
      lastSignificant = ''
      continue
    }

    // Block comment.
    if (grammar.blockComment !== null) {
      const [open, close] = grammar.blockComment
      if (startsWith(code, i, open)) {
        const end = code.indexOf(close, i + open.length)
        const stop = end === -1 ? n : end + close.length
        push('comment', code.slice(i, stop))
        i = stop
        lastSignificant = ''
        continue
      }
    }

    // Multiline strings (template literals / triple quotes).
    let matchedString = false
    for (const delim of grammar.multilineStrings) {
      if (startsWith(code, i, delim)) {
        let j = i + delim.length
        while (j < n && !startsWith(code, j, delim)) {
          if (code.charAt(j) === '\\') j += 2
          else j += 1
        }
        const stop = Math.min(n, j + delim.length)
        push('string', code.slice(i, stop))
        i = stop
        matchedString = true
        break
      }
    }
    if (matchedString) {
      lastSignificant = 'string'
      continue
    }

    // Single-line strings.
    for (const delim of grammar.strings) {
      const quote = delim.charAt(0)
      if (startsWith(code, i, delim)) {
        let j = i + delim.length
        while (j < n && code.charAt(j) !== quote && code.charAt(j) !== '\n') {
          if (code.charAt(j) === '\\') j += 2
          else j += 1
        }
        const stop = j < n && code.charAt(j) === quote ? j + 1 : j
        const text = code.slice(i, stop)
        const isKey = grammar.jsonKeys && peekNonSpace(code, stop) === ':'
        push(isKey ? 'property' : 'string', text)
        i = stop
        matchedString = true
        break
      }
    }
    if (matchedString) {
      lastSignificant = 'string'
      continue
    }

    // Shell variables: $name or ${...}.
    if (grammar.shellVariables && ch === '$') {
      let j = i + 1
      if (code.charAt(j) === '{') {
        const end = code.indexOf('}', j)
        j = end === -1 ? n : end + 1
      } else {
        while (j < n && isIdentifierPart(code.charAt(j))) j += 1
      }
      push('property', code.slice(i, j))
      i = j
      lastSignificant = 'property'
      continue
    }

    // Numbers.
    if (
      isDigit(ch) ||
      (ch === '.' && i + 1 < n && isDigit(code.charAt(i + 1)))
    ) {
      const rest = code.slice(i)
      const m = rest.match(
        /^(0[xX][0-9a-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|\d[\d_]*\.?[\d_]*([eE][+-]?\d+)?)[a-zA-Z]*/,
      )
      const text = m?.[0] ?? ch
      push('number', text)
      i += text.length
      lastSignificant = 'number'
      continue
    }

    // Identifiers / keywords.
    if (isIdentifierStart(ch)) {
      let j = i + 1
      while (j < n && isIdentifierPart(code.charAt(j))) j += 1
      const name = code.slice(i, j)
      const followedByCall = peekNonSpace(code, j) === '('
      const precededByDot = lastSignificant === '.'
      push(classifyIdentifier(grammar, name, followedByCall, precededByDot), name)
      i = j
      lastSignificant = name
      continue
    }

    // Punctuation / operators (single char).
    if (PUNCTUATION.has(ch)) {
      push('punctuation', ch)
      i += 1
      lastSignificant = ch
      continue
    }
    if (OPERATOR.has(ch)) {
      push('operator', ch)
      i += 1
      lastSignificant = ch
      continue
    }

    // Anything else → plain.
    plain += ch
    i += 1
  }

  flush()
  return tokens
}

// Tokenize and split into per-line token rows (newlines are dropped from the
// emitted text; the renderer reintroduces line breaks structurally). A trailing
// newline yields a trailing empty row, matching editor line-count semantics.
export const tokenizeCodeLines = (
  code: string,
  language?: string,
): ReadonlyArray<ReadonlyArray<CodeToken>> => {
  const tokens = tokenizeCode(code, language)
  const lines: CodeToken[][] = []
  let current: CodeToken[] = []
  lines.push(current)

  for (const token of tokens) {
    const parts = token.text.split('\n')
    parts.forEach((part, index) => {
      if (index > 0) {
        current = []
        lines.push(current)
      }
      if (part.length > 0) {
        current.push({ kind: token.kind, text: part })
      }
    })
  }

  return lines
}

// --- Unified-diff parsing -----------------------------------------------------
//
// Pure (Foldkit-free) so both the web `diff` component and non-Foldkit hosts
// (e.g. the desktop transcript renderer) share one parser and stay byte-faithful
// to the same change model.

export type DiffRowKind = 'hunk' | 'add' | 'remove' | 'context'

export type DiffRow = {
  readonly kind: DiffRowKind
  readonly oldNo?: number
  readonly newNo?: number
  readonly text: string
}

export type ParsedDiff = {
  readonly rows: ReadonlyArray<DiffRow>
  readonly added: number
  readonly removed: number
  readonly filename?: string
}

const stripDiffPathPrefix = (path: string): string =>
  path.startsWith('a/') || path.startsWith('b/') ? path.slice(2) : path

// Parse a unified diff (git patch) into render rows. Header noise
// (`diff --git`, `index`, `---`, `+++`) is consumed for the filename but not
// rendered, leaving a clean hunk/line view.
export const parseUnifiedDiff = (
  patch: string,
  filenameOverride?: string,
): ParsedDiff => {
  const lines = patch.split('\n')
  const rows: DiffRow[] = []
  let oldNo = 0
  let newNo = 0
  let added = 0
  let removed = 0
  let filename = filenameOverride

  lines.forEach((line, index) => {
    // Drop a single trailing empty line produced by a trailing newline.
    if (line === '' && index === lines.length - 1) {
      return
    }

    if (line.startsWith('@@')) {
      const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/)
      if (match !== null) {
        oldNo = Number(match[1])
        newNo = Number(match[2])
      }
      rows.push({ kind: 'hunk', text: line })
      return
    }

    if (
      line.startsWith('diff ') ||
      line.startsWith('index ') ||
      line.startsWith('new file') ||
      line.startsWith('deleted file') ||
      line.startsWith('similarity ') ||
      line.startsWith('rename ') ||
      line.startsWith('\\')
    ) {
      return
    }

    if (line.startsWith('+++ ')) {
      const path = line.slice(4).trim()
      if (filename === undefined && path !== '/dev/null' && path.length > 0) {
        filename = stripDiffPathPrefix(path)
      }
      return
    }
    if (line.startsWith('--- ')) {
      const path = line.slice(4).trim()
      if (filename === undefined && path !== '/dev/null' && path.length > 0) {
        filename = stripDiffPathPrefix(path)
      }
      return
    }

    const sign = line.charAt(0)
    if (sign === '+') {
      added += 1
      rows.push({ kind: 'add', newNo, text: line.slice(1) })
      newNo += 1
      return
    }
    if (sign === '-') {
      removed += 1
      rows.push({ kind: 'remove', oldNo, text: line.slice(1) })
      oldNo += 1
      return
    }

    const text = line.startsWith(' ') ? line.slice(1) : line
    rows.push({ kind: 'context', oldNo, newNo, text })
    oldNo += 1
    newNo += 1
  })

  return {
    rows,
    added,
    removed,
    ...(filename === undefined ? {} : { filename }),
  }
}
