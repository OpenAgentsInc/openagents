import { UIMessage } from './types';
import { dummyMessages } from './dummyData'
import { useChat as vercelUseChat } from "@ai-sdk/react"

export function useChat(options: Parameters<typeof vercelUseChat>[0] = {}) {
  return vercelUseChat({
    ...options,
    initialMessages: dummyMessages as UIMessage[]
  })
}
