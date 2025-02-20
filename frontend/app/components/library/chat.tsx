import { ChatInput } from "../chat/chat-input"
import { Thinking } from "../chat/thinking"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "../ui/card"

export function ChatComponents() {
  return (
    <div className="grid gap-8">
      {/* Chat Input */}
      <Card>
        <CardHeader>
          <CardTitle>Chat Input</CardTitle>
          <CardDescription>Message input with GitHub repository selector</CardDescription>
        </CardHeader>
        <CardContent>
          <ChatInput />
        </CardContent>
      </Card>

      {/* Thinking */}
      <Card>
        <CardHeader>
          <CardTitle>Thinking Indicator</CardTitle>
          <CardDescription>Expandable component showing AI's chain of thought process during response generation</CardDescription>
        </CardHeader>
        <CardContent>
          <Thinking />
        </CardContent>
      </Card>
    </div>
  );
}
