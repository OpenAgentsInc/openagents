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
              <div role="radiogroup" aria-required="false" dir="ltr" className="flex w-full overflow-hidden rounded-xl bg-token-surface-secondary p-1.5 dark:bg-token-surface-tertiary md:w-1/2 mb-2 flex-shrink-0 self-center" tabIndex={0} style={{ outline: 'none' }}>
                <button type="button" role="radio" aria-checked="true" data-state="checked" value="magic" className="text-md w-1/3 flex-grow rounded-lg border-token-border-light p-1.5 font-medium text-token-text-tertiary transition hover:text-token-text-primary radix-state-checked:border radix-state-checked:bg-token-surface-primary radix-state-checked:text-token-text-primary radix-state-checked:shadow-[0_0_2px_rgba(0,0,0,.03)] radix-state-checked:dark:bg-token-surface-secondary md:w-1/2" tabIndex={0} data-radix-collection-item="">
                  Create
                </button>
                <button type="button" role="radio" aria-checked="false" data-state="unchecked" value="advanced" className="text-md w-1/3 flex-grow rounded-lg border-token-border-light p-1.5 font-medium text-token-text-tertiary transition hover:text-token-text-primary radix-state-checked:border radix-state-checked:bg-token-surface-primary radix-state-checked:text-token-text-primary radix-state-checked:shadow-[0_0_2px_rgba(0,0,0,.03)] radix-state-checked:dark:bg-token-surface-secondary md:w-1/2" tabIndex={-1} data-radix-collection-item="">Configure
                </button>
                <div className="flex w-1/3 md:hidden"><button type="button" role="radio" aria-checked="false" data-state="unchecked" value="preview" className="text-md w-1/3 flex-grow rounded-lg border-token-border-light p-1.5 font-medium text-token-text-tertiary transition hover:text-token-text-primary radix-state-checked:border radix-state-checked:bg-token-surface-primary radix-state-checked:text-token-text-primary radix-state-checked:shadow-[0_0_2px_rgba(0,0,0,.03)] radix-state-checked:dark:bg-token-surface-secondary md:w-1/2" tabIndex={-1} data-radix-collection-item="">
                  Preview
                </button>
                </div>
              </div>
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
