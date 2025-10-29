import React from 'react'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'

export function ContentText({ text }: { text: string }) {
  return <MarkdownBlock markdown={text} />
}

