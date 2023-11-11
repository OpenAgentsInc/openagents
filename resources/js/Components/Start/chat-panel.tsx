import { ButtonScrollToBottom } from './button-scroll-to-bottom'
import { Button } from '@/Components/ui/button'
import { IconRefresh, IconStop } from '@/Components/ui/icons'
import { PromptForm } from './prompt-form'

export interface ChatPanelProps
// extends Pick<
//   UseChatHelpers,
//   'append' | 'isLoading' | 'reload' | 'messages' | 'stop' | 'input' | 'setInput'
// > {
{
  id?: string
  containerRef: React.RefObject<HTMLElement>
}

export function ChatPanel({
  id,
  isLoading,
  stop,
  append,
  reload,
  messages,
  input,
  setInput,
  containerRef
}: any) {
  return (
    <div className="sticky bottom-0 pb-2">
      {/* <ButtonScrollToBottom containerRef={containerRef} /> */}
      <div className="mx-auto sm:max-w-2xl sm:px-4">
        <div className="flex h-10 items-center justify-end">
          {isLoading ? (
            <Button variant="outline" onClick={() => stop()} className="bg-background">
              <IconStop className="mr-2" />
              Stop generating
            </Button>
          ) : (
            messages?.length > 0 && (
              <Button variant="outline" onClick={() => reload()} className="bg-background">
                <IconRefresh className="mr-2" />
                Regenerate
              </Button>
            )
          )}
        </div>
        <div className="space-y-4 border-t bg-background px-4 py-2 shadow-lg sm:rounded-t-xl sm:border md:py-4 w-full">
          <PromptForm
            onSubmit={async value => {
              await append({
                id,
                content: value,
                role: 'user'
              })

              // updateBalances()
            }}
            input={input}
            setInput={setInput}
            isLoading={isLoading}
          />
        </div>
      </div>
    </div>
  )
}
