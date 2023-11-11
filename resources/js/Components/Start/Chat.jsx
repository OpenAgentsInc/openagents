import React from "react"
import { EmptyScreen } from "./empty-screen"
import { ChatPanel } from "./chat-panel"

export function Chat() {
  const id = 1
  const isLoading = false
  const append = (what) => {
    console.log('append', what)
  }
  const reload = () => { }
  const messages = []
  const [input, setInput] = React.useState('')
  const containerRef = React.useRef(null)
  return (
    <div className="h-screen">
    <div
      className="flex flex-col relative pl-[2rem] z-1 w-full overflow-y-auto h-full"
      ref={containerRef}
    >
      <div className='flex-1 pt-4 md:pt-10 pb-4'>
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
