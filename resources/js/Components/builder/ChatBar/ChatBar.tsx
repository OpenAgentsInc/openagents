import { useEnterSubmit } from '@/lib/hooks/use-enter-submit'
import axios from 'axios'
import { useCallback, useState } from 'react'
import Textarea from 'react-textarea-autosize'

export const ChatBar = ({ messages, setMessages }) => {
  const { formRef, onKeyDown } = useEnterSubmit()
  const [input, setInput] = useState('')
  const isLoading = false
  const onSubmit = useCallback(async (input: string) => {
    // axios.post('/stream', { input })

    axios.post('/agent/1/chat', { input })
      // then console log the response
      .then(function (response) {
        // console.log(response);
      })
      // catch any errors
      .catch(function (error) {
        console.log(error);
      });


    setMessages([...messages, { role: "user", content: input }])
  }, [messages])
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
      <div className="shadow-lg relative flex h-full w-full cursor-text items-end border border-transparent bg-neutral-25 transition-all duration-300 focus-within:border-neutral-400 focus-within:shadow-none hover:border-neutral-400 hover:shadow-none rounded-[30px]">
        <div className="relative my-1.5 ml-1.5 z-10">
          {/* <button type="button" className="grid h-10 w-12 place-items-center rounded-full transition-colors duration-300 bg-neutral-200 hover:bg-neutral-200-hover active:bg-neutral-200-tap"></button> */}
        </div>
        <div className="h-full grow overflow-y-auto py-3 pr-4 lg:py-[5px] 2xl:py-[8.5px] pl-2">
          <Textarea
            tabIndex={0}
            onKeyDown={onKeyDown}
            rows={1}
            value={input}
            onChange={e => setInput(e.target.value)}
            autoFocus
            className="t-body-chat block w-full resize-none overflow-y-hidden whitespace-pre-wrap bg-transparent text-primary-700 outline-none placeholder:text-neutral-600" spellCheck="false" placeholder="Say something..." style={{ height: 32 }}
          />
        </div>
        <button aria-label="Submit text"
          className="bg-button disabled:bg-neutral-50 disabled:opacity-50 shadow flex h-9 w-9 items-center justify-center rounded-full p-1.5 text-neutral-600 m-2 duration-300 transition-opacity"
          type="submit"
          disabled={isLoading || input === ""}>
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="16" fill="currentColor"><path fillRule="evenodd" d="M.852 7.648a1.2 1.2 0 0 1 0-1.696l4.8-4.8a1.2 1.2 0 0 1 1.696 0l4.8 4.8a1.2 1.2 0 1 1-1.697 1.696L7.7 4.897V14a1.2 1.2 0 0 1-2.4 0V4.897L2.548 7.648a1.2 1.2 0 0 1-1.696 0Z" clipRule="evenodd"></path></svg>
        </button>
      </div>
    </form>
  )
}
