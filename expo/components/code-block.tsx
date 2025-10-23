import React from 'react'
import { View, Text } from 'react-native'
import { Highlight, themes } from 'prism-react-renderer'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

function normalizeLanguage(lang?: string): string {
  const s = String(lang || '').trim().toLowerCase()
  if (!s) return 'tsx'
  if (['sh', 'bash', 'zsh', 'shell', 'console'].includes(s)) return 'bash'
  if (['js', 'javascript', 'node'].includes(s)) return 'javascript'
  if (['ts', 'typescript'].includes(s)) return 'tsx'
  if (['json', 'json5'].includes(s)) return 'json'
  if (['md', 'markdown'].includes(s)) return 'markdown'
  if (['py', 'python'].includes(s)) return 'python'
  return s
}

export function CodeBlock({ code, language, maxHeight }: { code: string; language?: string; maxHeight?: number }) {
  const lang = normalizeLanguage(language)
  const lines = String(code ?? '').replace(/\r\n/g, '\n')
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.black, maxHeight, overflow: 'hidden' as any }}>
      <Highlight theme={themes.vsDark} code={lines} language={lang as any}>
        {({ tokens, getLineProps, getTokenProps }) => (
          <View style={{ padding: 8 }}>
            {tokens.map((line, i) => (
              <Text key={i} style={{ fontFamily: Typography.primary, color: Colors.foreground, lineHeight: 16 }} {...(getLineProps({ line }) as any)}>
                {line.map((token, key) => {
                  const props = getTokenProps({ token }) as any
                  const color = props?.style?.color
                  return (
                    <Text key={key} style={{ color: color || Colors.foreground, fontFamily: Typography.primary }}>
                      {props.children}
                    </Text>
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

