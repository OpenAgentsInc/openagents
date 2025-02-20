import { useEffect, useState } from "react"
import { ChatInput } from "../chat/chat-input"
import { Thinking } from "../chat/thinking"
import { Button } from "../ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "../ui/card"

import type { ThinkingState } from "../chat/thinking"
const LOREM_IPSUM = `Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.`;

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
          <div className="space-y-4">
            <Button onClick={startThinking} disabled={isThinking}>
              Start Demo Thinking
            </Button>
            <Thinking
              state={thinkingState}
              duration={duration}
              content={content}
            />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
