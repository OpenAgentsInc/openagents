import { UIMessage } from './types';
import { dummyMessages } from './dummyData'

export function useChat() {
  return {
    messages: dummyMessages as UIMessage[]
  }
}
