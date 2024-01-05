import { ChatPane } from "../ChatPane"

export const AgentBuilder = () => {
  return (
    <div className="flex h-screen w-full flex-col items-center">
      <div className="relative flex h-14 w-full items-center justify-between gap-2 border-b border-token-border-medium px-3 flex-shrink-0">
        <p>Agent Builder</p>
      </div>
      <div className="relative flex w-full grow overflow-hidden">
        <div className="flex w-full justify-center md:w-1/2">
          <div className="h-full grow overflow-hidden">
            <div className="flex h-full flex-col px-2 pt-2">
              <ChatPane conversationId={1} />
            </div>
          </div>
        </div>
        <div className="hidden w-1/2 justify-center border-l pt-2 md:flex">
          <ChatPane conversationId={1} />
        </div>
      </div>
    </div>
  )
}
