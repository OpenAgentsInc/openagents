import { MessageList, UIMessage } from "@openagents/ui";

export default function ChatPage() {
  const messages: UIMessage[] = [{
    id: "1",
    role: "user",
    content: "Hello, how are you?",
    createdAt: new Date(),
    parts: [{
      type: "text",
      text: "Hello, how are you?"
    }]
  }, {
    id: "2",
    role: "assistant",
    content: "I'm good, thank you!",
    createdAt: new Date(),
    parts: [{
      type: "text",
      text: "I'm good, thank you!"
    }]
  }]
  // const { messages } = useOpenAgent('coder')
  return (
    <MessageList messages={messages} />
  )
}
