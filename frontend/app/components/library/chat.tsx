import { ChatInput } from "../chat/chat-input"
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
    </div>
  );
}
