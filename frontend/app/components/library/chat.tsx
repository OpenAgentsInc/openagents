import { useEffect, useState } from "react"
import { ChatInput } from "../chat/chat-input"
import { Thinking } from "../chat/thinking"
import { Button } from "../ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "../ui/card"

import type { ThinkingState } from "../chat/thinking"
const LOREM_IPSUM = `Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.`;

const EXAMPLE_CONTENT = [
  "1. Analyzing the request and breaking it down into steps",
  "2. Searching relevant documentation and context",
  "3. Formulating a response based on gathered information",
  "4. Checking for potential edge cases and errors",
  "5. Optimizing the solution for better performance",
];

export function ChatComponents() {
  const [isThinking, setIsThinking] = useState(false);
  const [thinkingState, setThinkingState] = useState<ThinkingState>("finished");
  const [duration, setDuration] = useState(0);
  const [content, setContent] = useState<string[]>([]);
  const [streamedText, setStreamedText] = useState("");

  useEffect(() => {
    if (isThinking) {
      let startTime = Date.now();
      let textIndex = 0;

      const timer = setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTime) / 1000);
        setDuration(elapsed);

        if (elapsed >= 5) {
          clearInterval(timer);
          setIsThinking(false);
          setThinkingState("finished");
          setStreamedText(LOREM_IPSUM);
          return;
        }

        textIndex += 5;
        setStreamedText(LOREM_IPSUM.slice(0, textIndex));
      }, 100);

      return () => clearInterval(timer);
    }
  }, [isThinking]);

  useEffect(() => {
    if (streamedText) {
      setContent([streamedText]);
    }
  }, [streamedText]);

  const startThinking = () => {
    setIsThinking(true);
    setThinkingState("thinking");
    setDuration(0);
    setStreamedText("");
    setContent([]);
  };

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
              <h3 className="text-lg font-medium">Interactive Demo</h3>
              <Button onClick={startThinking} disabled={isThinking}>
                Start Demo Thinking
              </Button>
              <Thinking
                state={thinkingState}
                duration={duration}
                content={content}
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
