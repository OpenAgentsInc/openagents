import React from 'react'

// Render JSON with minimal syntax highlighting using our theme variables.
// - Keys: var(--secondary)
// - Strings: var(--success)
// - Numbers: var(--warning)
// - Booleans: var(--danger)
// - null: var(--tertiary)

export function renderJsonSyntax(input: string): React.ReactNode | null {
  let json: string
  try {
    const parsed = JSON.parse(input)
    json = JSON.stringify(parsed, null, 2)
  } catch {
    return null
  }

  const re = /(\"(?:\\.|[^\\\"])*\"\s*:)|(\"(?:\\.|[^\\\"])*\")|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+\-]?\d+)?)/g
  const nodes: React.ReactNode[] = []
  let last = 0
  let key = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(json)) !== null) {
    const idx = m.index
    if (idx > last) {
      nodes.push(<span key={`t${key++}`}>{json.slice(last, idx)}</span>)
    }
    const [token, keyStr, strVal, boolNull, numberVal] = m
    if (keyStr) {
      nodes.push(<span key={`k${key++}`} className="text-[var(--secondary)]">{keyStr}</span>)
    } else if (strVal) {
      nodes.push(<span key={`s${key++}`} className="text-[var(--success)]">{strVal}</span>)
    } else if (boolNull) {
      const cls = boolNull === 'null' ? 'text-[var(--tertiary)]' : 'text-[var(--danger)]'
      nodes.push(<span key={`b${key++}`} className={cls}>{boolNull}</span>)
    } else if (numberVal) {
      nodes.push(<span key={`n${key++}`} className="text-[var(--warning)]">{numberVal}</span>)
    } else {
      nodes.push(<span key={`x${key++}`}>{token}</span>)
    }
    last = idx + token.length
  }
  if (last < json.length) {
    nodes.push(<span key={`t${key++}`}>{json.slice(last)}</span>)
  }
  return <>{nodes}</>
}

