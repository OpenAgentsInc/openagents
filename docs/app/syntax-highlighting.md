# Syntax Highlighting in the App (React Native)

This app uses Prism React Renderer to syntax‑highlight code blocks in React Native screens. This doc covers how to install it, how we render highlighted code, how to hook it into Markdown, how to load themes and languages, and a few gotchas for mobile.

## Why Prism React Renderer
- Works in React and React Native (no DOM requirement).
- Ships with a vendored Prism that avoids global pollution.
- Simple render‑props API gives full control over the RN `<Text>` tree.

## Install

We treat Prism React Renderer as a generic JS dependency.

- Command:
  - `cd expo && bun add prism-react-renderer`

Notes
- Don’t pin versions manually; commit the updated `bun.lock`.
- No native modules required; works out of the box on iOS/Android/Web.

## Basic Usage (CodeBlock component)

Below is a minimal RN code block that renders highlighted code using `<Highlight />`. It converts tokens → nested `<Text>` elements so selection and copying work natively.

```tsx
import React from 'react'
import { View, Text } from 'react-native'
import { Highlight, themes } from 'prism-react-renderer'

export function CodeBlock({ code, language = 'tsx' }: { code: string; language?: string }) {
  return (
    <View style={{ borderWidth: 1, borderColor: '#23252a', backgroundColor: '#000' }}>
      <Highlight theme={themes.vsDark} code={code.replace(/\r\n/g, '\n')} language={language as any}>
        {({ tokens, getLineProps, getTokenProps }) => (
          <View style={{ padding: 8 }}>
            {tokens.map((line, i) => (
              <Text key={i} style={{ lineHeight: 16 }} {...(getLineProps({ line }) as any)}>
                {line.map((token, key) => {
                  const props = getTokenProps({ token }) as any
                  const color = props?.style?.color
                  return (
                    <Text key={key} style={{ color }}>{props.children}</Text>
                  )
                })}
              </Text>
            ))}
          </View>
        )}
      </Highlight>
    </View>
  )
}
```

Tips
- Replace CRLF with LF to avoid odd wrapping on iOS.
- Use your app’s monospace font via your typography setup (e.g., Berkeley Mono) if desired.

## Hooking Into Markdown

We render Markdown via `react-native-markdown-display`. To use Prism for fenced code blocks and indented code blocks, supply `rules` that hand off to `CodeBlock`.

```tsx
import Markdown from 'react-native-markdown-display'
import { CodeBlock } from '@/components/code-block'

export function MarkdownBlock({ markdown }: { markdown: string }) {
  const rules: any = {
    fence: (node: any) => (
      <CodeBlock code={String(node?.content ?? '')} language={String((node?.params ?? node?.info) || '')} />
    ),
    code_block: (node: any) => (
      <CodeBlock code={String(node?.content ?? '')} />
    ),
  }
  return (
    <Markdown rules={rules}>{markdown}</Markdown>
  )
}
```

Where used
- Agent messages that contain Markdown.
- Reasoning cards that may include inline/fenced code.
- Command output previews (e.g., stdout/stderr samples).

## Current Integration Points (repo paths)

These files already use Prism‑based highlighting:

- Code renderer: `expo/components/code-block.tsx`
- Markdown (agent output): `expo/components/jsonl/MarkdownBlock.tsx`
- Reasoning headline: `expo/components/jsonl/ReasoningHeadline.tsx`
- Reasoning card: `expo/components/jsonl/ReasoningCard.tsx`
- Command output samples: `expo/components/jsonl/CommandExecutionCard.tsx`
- Message detail raw JSON: `expo/app/message/[id].tsx`

Notes
- For raw JSON in message detail, we set `language="json"` to apply JSON highlighting to the full payload.
- Markdown fence rules pass the fence’s info string (e.g., `tsx`, `bash`) down as the language.

## Themes

Prism React Renderer exports a set of built‑in themes via `themes`. Example:

```tsx
import { Highlight, themes } from 'prism-react-renderer'

<Highlight theme={themes.vsDark} code={code} language="tsx">{...}</Highlight>
```

Popular choices: `themes.vsDark`, `themes.shadesOfPurple`, `themes.nightOwl`, `themes.dracula`.

Custom theme
- You can provide a `PrismTheme` object instead of a built‑in theme. See the README for the structure or copy one from the package and tweak colors.

Swap theme globally
- We currently import `themes.vsDark` directly in `CodeBlock`.
- To change globally, update the `theme` prop in `expo/components/code-block.tsx` to a different built‑in theme, or export a central theme constant and reuse it.

```tsx
// expo/components/code-block.tsx
import { Highlight, themes } from 'prism-react-renderer'
const THEME = themes.nightOwl // choose your theme here
...
<Highlight theme={THEME} code={lines} language={lang as any}>
```

## Languages

Out of the box, Prism React Renderer bundles a base set of languages. For additional languages, load language definitions from `prismjs` and attach the vendored Prism to the global before importing the components.

```ts
import { Highlight, Prism } from 'prism-react-renderer'
;(typeof global !== 'undefined' ? (global as any) : (window as any)).Prism = Prism
await import('prismjs/components/prism-applescript')
// Repeat for other languages as needed
```

Alternatively, pass a custom Prism instance:

```tsx
import Prism from 'prismjs/components/prism-core'
<Highlight prism={Prism} code={code} language="applescript">{...}</Highlight>
```

Language detection
- We map common aliases to bundled languages (e.g., `sh|bash|zsh → bash`, `js → javascript`, `ts → tsx`, `json5 → json`).
- When rendering from Markdown, the fence info string (```tsx, ```bash, ```json) becomes the `language` prop.

Expo/Metro note
- Dynamic language imports work in Expo via `await import('prismjs/components/prism-…')` as long as you keep the `global.Prism` assignment before the imports.
- Add these imports near app startup or lazily where needed (e.g., within a screen effect) to avoid slowing initial load.

## Mobile Considerations

- Performance: Avoid tokenizing very large blobs inline. If needed, cap height and let users expand on demand.
- Font: Use a monospace font for better legibility; we default to the app’s mono font.
- Copy to clipboard: Wrap blocks in a `Pressable` and use `expo-clipboard` to copy on long‑press.
- Selection: RN `<Text selectable>` enables text selection. We rely on nested `<Text>`; don’t intermix `<View>` within a line.
- Colors: Themes provide inline `style` props with colors — no CSS-in-JS required.

## Example: Full Featured CodeBlock

```tsx
import React from 'react'
import { View, Text } from 'react-native'
import { Highlight, themes } from 'prism-react-renderer'

function normalizeLanguage(lang?: string): string {
  const s = String(lang || '').trim().toLowerCase()
  if (!s) return 'tsx'
  if (['sh','bash','zsh','shell','console'].includes(s)) return 'bash'
  if (['js','javascript','node'].includes(s)) return 'javascript'
  if (['ts','typescript'].includes(s)) return 'tsx'
  if (['json','json5'].includes(s)) return 'json'
  if (['md','markdown'].includes(s)) return 'markdown'
  if (['py','python'].includes(s)) return 'python'
  return s
}

export function CodeBlock({ code, language, maxHeight }: { code: string; language?: string; maxHeight?: number }) {
  const lang = normalizeLanguage(language)
  const lines = String(code ?? '').replace(/\r\n/g, '\n')
  return (
    <View style={{ borderWidth: 1, borderColor: '#23252a', backgroundColor: '#000', maxHeight, overflow: 'hidden' as any }}>
      <Highlight theme={themes.vsDark} code={lines} language={lang as any}>
        {({ tokens, getLineProps, getTokenProps }) => (
          <View style={{ padding: 8 }}>
            {tokens.map((line, i) => (
              <Text key={i} style={{ lineHeight: 16 }} {...(getLineProps({ line }) as any)}>
                {line.map((token, key) => {
                  const props = getTokenProps({ token }) as any
                  const color = props?.style?.color
                  return (
                    <Text key={key} style={{ color }}>{props.children}</Text>
                  )
                })}
              </Text>
            ))}
          </View>
        )}
      </Highlight>
    </View>
  )
}
```

## Testing

- Typecheck: `cd expo && bun run typecheck`
- Run app: `cd expo && bun run start`
- Render a few fenced blocks (```tsx, ```bash, ```json) to verify highlighting and wrapping.

## References
- prism-react-renderer README: https://github.com/FormidableLabs/prism-react-renderer#readme
- Prism languages: https://prismjs.com/#languages-list
- react-native-markdown-display: https://github.com/iamacup/react-native-markdown-display
