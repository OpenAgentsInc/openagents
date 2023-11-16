import React from "react"
import { EmptyScreen } from "./empty-screen"
import { ChatPanel } from "./chat-panel"
import { ChatList } from "./chat-list"
import { ChatScrollAnchor } from "./chat-scroll-anchor"
import { usePage } from '@inertiajs/react'
import axios from "axios"

export function Chat() {
  const id = 1
  const isLoading = false
  const props = usePage().props
  const reload = () => { }
  const [messages, setMessages] = React.useState([])
  const [input, setInput] = React.useState('')
  const append = (what) => {

    // Add the user's message to the chat
    setMessages((messages) => [...messages, what]);

    // Send an axios POST request to /api/query with the user's message and corpus ID of 4
    axios.post('/api/query', {
      query: what.content,
      file_id: props.flash?.filename ?? null
    })
      .then(function (response) {
        console.log(response)
        if (response.data && response.data.ok) {
          // Create a new message object for the response
          const responseMessage = {
            id: messages.length + 1, // Assuming ID is just the next number in sequence
            content: response.data.results[0].text,
            // content: response.data.summary,
            role: 'assistant' // or any appropriate role for the response
          };

          // Append the response message to the messages state
          setMessages((messages) => [...messages, responseMessage]);
        }
      })
      .catch(function (error) {
        console.log(error);
      });
  }
  // console.log(messages)
  const containerRef = React.useRef(null)
  return (
    <div className="h-screen">
      <div
        className="flex flex-col relative pl-[2rem] z-1 w-full overflow-y-auto h-full"
        ref={containerRef}
      >
        <div className='w-[600px] flex-1 pt-4 md:pt-10 pb-4'>
          {messages.length ? (
            <>
              <ChatList messages={messages} />
              <ChatScrollAnchor trackVisibility={isLoading} />
            </>
          ) : (
            <EmptyScreen />
          )}
        </div>

        <ChatPanel
          id={id}
          isLoading={isLoading}
          stop={stop}
          append={append}
          reload={reload}
          messages={messages}
          input={input}
          setInput={setInput}
          containerRef={containerRef}
        />

        {/* <PreviewToken /> */}
      </div>
    </div>
  )
}
