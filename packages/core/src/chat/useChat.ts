import { UIMessage } from './types';
import { dummyMessages } from './dummyData'
import { useChat as vercelUseChat } from "@ai-sdk/react"

export function useChat(options: Parameters<typeof vercelUseChat>[0] = {}) {
  return vercelUseChat({
    ...options,
    api: "https://chat.openagents.com",
    onError: (error) => {
      console.error('Chat error:', error);
      options.onError?.(error);
    },
    // initialMessages: dummyMessages as UIMessage[],
    // fetch: (input: RequestInfo | URL, init?: RequestInit) => fetch(input, { ...init, mode: 'no-cors' })
  })
}
