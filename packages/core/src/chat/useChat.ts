import { UIMessage } from './types';

export function useChat() {
  return {
    messages: [] as UIMessage[]
  }
}
