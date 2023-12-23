import { useEnterSubmit } from '@/lib/hooks/use-enter-submit'
import { useCallback, useState } from 'react'
import Textarea from 'react-textarea-autosize'

export const PromptForm = () => {
  const { formRef, onKeyDown } = useEnterSubmit()
  const [input, setInput] = useState('')
  const onSubmit = useCallback(async (input: string) => {
    console.log(input)
  }, [])
  return (
    <form
      onSubmit={async e => {
        e.preventDefault()
        if (!input?.trim()) {
          return
        }
        setInput('')
        await onSubmit(input)
      }}
      ref={formRef}
    >
      <Textarea
        tabIndex={0}
        onKeyDown={onKeyDown}
        rows={1}
        value={input}
        onChange={e => setInput(e.target.value)}
        placeholder="Say something..."
        className="t-body-chat block w-full resize-none overflow-y-hidden whitespace-pre-wrap bg-transparent text-primary-700 outline-none placeholder:text-neutral-600" spellCheck="false" placeholder="Say something..." style={{ height: 32 }}
      />
    </form>
  )
}
