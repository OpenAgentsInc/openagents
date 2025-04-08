import { UIMessage } from "@openagents/ui";

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
  console.log(messages)
  return (
    <div className="mt-10 flex flex-col gap-4">
      {messages.map((message) => (
        <div key={message.id}>
          {message.parts.map((part, i) => (
            <p key={part.type + i} className="text-white">{part.text ?? ""}</p>
          ))}
        </div>
      ))}
    </div>
  )
}
