import { Link } from "react-router"
import { ChatInput } from "../chat/chat-input"
import { Thinking } from "../chat/thinking"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "../ui/card"

const EXAMPLE_CONTENT = [
  "1. Analyzing the request and breaking it down into steps",
  "2. Searching relevant documentation and context",
  "3. Formulating a response based on gathered information",
  "4. Checking for potential edge cases and errors",
  "5. Optimizing the solution for better performance",
];

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
          <CardDescription>Expandable component showing AI's chain of thought process with duration tracking</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-8 px-8">
            {/* Interactive Demo */}
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="text-lg font-medium">Interactive Demo</h3>
                <Link
                  to="/thinking"
                  className="text-sm text-muted-foreground hover:text-primary transition-colors"
                >
                  View full page demo →
                </Link>
              </div>
              <Thinking
                state="thinking"
                duration={3}
                content={EXAMPLE_CONTENT}
                defaultOpen={true}
              />
            </div>

            {/* Static Examples */}
            <div className="space-y-4">
              <h3 className="text-lg font-medium">Static Examples</h3>
              <div className="grid gap-4">
                <div>
                  <h4 className="text-sm font-medium mb-2 text-muted-foreground">Thinking State</h4>
                  <Thinking
                    state="thinking"
                    duration={3}
                    content={EXAMPLE_CONTENT}
                  />
                </div>
                <div>
                  <h4 className="text-sm font-medium mb-2 text-muted-foreground">Finished State</h4>
                  <Thinking
                    state="finished"
                    duration={5}
                    content={EXAMPLE_CONTENT}
                  />
                </div>
                <div>
                  <h4 className="text-sm font-medium mb-2 text-muted-foreground">Error State</h4>
                  <Thinking
                    state="error"
                    duration={2}
                    content={["An error occurred while processing the request.", "Please try again or contact support if the issue persists."]}
                  />
                </div>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
