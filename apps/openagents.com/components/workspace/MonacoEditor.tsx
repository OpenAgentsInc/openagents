'use client'

import React, { useRef, useEffect, useState } from 'react'
import Editor, { Monaco } from '@monaco-editor/react'
import { cx } from '@arwes/react'

interface MonacoEditorProps {
  defaultValue?: string
  value?: string
  language?: string
  path?: string
  onChange?: (value: string | undefined) => void
  onMount?: (editor: any, monaco: Monaco) => void
  readOnly?: boolean
  className?: string
}

// Arwes-themed Monaco configuration
const ARWES_THEME = {
  base: 'vs-dark' as const,
  inherit: true,
  rules: [
    { token: 'comment', foreground: '608b90' },
    { token: 'keyword', foreground: '00ffff' },
    { token: 'string', foreground: '98fb98' },
    { token: 'number', foreground: 'ff00ff' },
    { token: 'type', foreground: '00d9ff' },
    { token: 'function', foreground: 'f7931a' },
    { token: 'variable', foreground: 'e0e0e0' },
    { token: 'constant', foreground: 'ff00ff' },
    { token: 'class', foreground: '00d9ff' },
    { token: 'interface', foreground: '00d9ff' },
    { token: 'namespace', foreground: '00d9ff' },
    { token: 'parameter', foreground: 'e0e0e0' },
    { token: 'property', foreground: '98fb98' },
    { token: 'tag', foreground: '00ffff' },
    { token: 'attribute.name', foreground: '98fb98' },
    { token: 'attribute.value', foreground: 'ff00ff' },
  ],
  colors: {
    'editor.background': '#000000',
    'editor.foreground': '#e0e0e0',
    'editor.lineHighlightBackground': '#0a1a1a',
    'editor.selectionBackground': '#00d9ff33',
    'editor.inactiveSelectionBackground': '#00d9ff1a',
    'editorCursor.foreground': '#00ffff',
    'editorWhitespace.foreground': '#404040',
    'editorIndentGuide.background': '#202020',
    'editorIndentGuide.activeBackground': '#303030',
    'editorLineNumber.foreground': '#406060',
    'editorLineNumber.activeForeground': '#00d9ff',
    'editorRuler.foreground': '#303030',
    'editor.wordHighlightBackground': '#00d9ff22',
    'editor.wordHighlightStrongBackground': '#00d9ff44',
    'editor.findMatchBackground': '#f7931a44',
    'editor.findMatchHighlightBackground': '#f7931a22',
    'editorBracketMatch.background': '#00ffff33',
    'editorBracketMatch.border': '#00ffff66',
    'editorGutter.background': '#000000',
    'editorGutter.modifiedBackground': '#00d9ff',
    'editorGutter.addedBackground': '#98fb98',
    'editorGutter.deletedBackground': '#ff0066',
    'scrollbar.shadow': '#00000000',
    'scrollbarSlider.background': '#00d9ff33',
    'scrollbarSlider.hoverBackground': '#00d9ff66',
    'scrollbarSlider.activeBackground': '#00d9ff99',
  }
}

export function MonacoEditor({
  defaultValue = '',
  value,
  language = 'typescript',
  path,
  onChange,
  onMount,
  readOnly = false,
  className = ''
}: MonacoEditorProps) {
  const monacoRef = useRef<Monaco | null>(null)

  const handleEditorMount = (editor: any, monaco: Monaco) => {
    // Store monaco reference
    monacoRef.current = monaco

    // Define and set the Arwes theme
    monaco.editor.defineTheme('arwes', ARWES_THEME)
    monaco.editor.setTheme('arwes')

    // Configure editor options
    editor.updateOptions({
      fontSize: 14,
      fontFamily: 'var(--font-berkeley-mono), "Berkeley Mono", monospace',
      lineHeight: 20,
      renderWhitespace: 'selection',
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      wordWrap: 'on',
      wrappingStrategy: 'advanced',
      glyphMargin: true,
      folding: true,
      lineNumbers: 'on',
      lineDecorationsWidth: 10,
      lineNumbersMinChars: 4,
      renderLineHighlight: 'all',
      scrollbar: {
        verticalScrollbarSize: 10,
        horizontalScrollbarSize: 10,
      },
      overviewRulerLanes: 0,
      hideCursorInOverviewRuler: true,
      overviewRulerBorder: false,
      automaticLayout: true,
    })

    // Call user's onMount if provided
    if (onMount) {
      onMount(editor, monaco)
    }
  }

  return (
    <div className={cx('w-full h-full relative', className)} style={{ minHeight: '400px' }}>
      <div className="absolute inset-0">
        <Editor
          height="100%"
          width="100%"
          defaultValue={defaultValue}
          value={value}
          language={language}
          path={path}
          theme="arwes"
          onChange={onChange}
          onMount={handleEditorMount}
          options={{
            readOnly,
            domReadOnly: readOnly,
            automaticLayout: true,
          }}
          loading={
            <div className="w-full h-full flex items-center justify-center bg-black" style={{ minHeight: '400px' }}>
              <div className="text-cyan-500 font-mono">Loading editor...</div>
            </div>
          }
        />
      </div>
    </div>
  )
}