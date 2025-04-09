import { type OpenAgent } from "@openagents/core"

export function AgentChat({ agent }: { agent: OpenAgent }) {
  console.log(agent)
  return (
    <div>
      <div className="h-full flex flex-col gap-4 justify-center items-center pt-24">
        {agent.messages.map((message) => (
          <div key={message.id}>
            {message.parts.map((part) => (
              <p key={part.type}>
                {part.type === 'text' && part.text}
                {part.type === 'reasoning' && part.reasoning}
                {part.type === 'file' && part.data}
              </p>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}
