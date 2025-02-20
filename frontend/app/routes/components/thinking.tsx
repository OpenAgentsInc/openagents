import { useEffect, useState } from "react";
import { Link } from "react-router";
import { Thinking } from "~/components/chat/thinking";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "~/components/ui/breadcrumb";
import { Button } from "~/components/ui/button";

// Proper Lorem Ipsum text with longer lines
const DEMO_TEXT = Array(10)
  .fill([
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore magna",
    "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat",
    "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla in tempus",
    "Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id laborum",
    "Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium totam rem",
    "Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia magni consequuntur",
    "At vero eos et accusamus et iusto odio dignissimos ducimus qui blanditiis praesentium voluptatum deleniti",
  ])
  .flat();

export default function ThinkingPage() {
  const [isThinking, setIsThinking] = useState(false);
  const [hasStarted, setHasStarted] = useState(false);
  const [thinkingState, setThinkingState] = useState<
    "thinking" | "finished" | "error"
  >("finished");
  const [duration, setDuration] = useState(0);
  const [animatedContent, setAnimatedContent] = useState<
    Array<{ text: string; opacity: number }>
  >([]);

  const allLines = DEMO_TEXT; // No need to split, using pre-split lines

  useEffect(() => {
    if (isThinking) {
      let startTime = Date.now();
      let currentLineIndex = 0;

      // Start with empty content
      setAnimatedContent([]);

      const timer = setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTime) / 1000);
        setDuration(elapsed);

        if (elapsed >= 10 || currentLineIndex >= allLines.length) {
          clearInterval(timer);
          setIsThinking(false);
          setThinkingState("finished");
          return;
        }

        // Add new line at the bottom (start of array)
        setAnimatedContent((prev) => [
          { text: allLines[currentLineIndex], opacity: 0 },
          ...prev,
        ]);

        // Fade in the latest line
        setTimeout(() => {
          setAnimatedContent((prev) =>
            prev.map((line, i) => (i === 0 ? { ...line, opacity: 1 } : line)),
          );
        }, 50);

        currentLineIndex++;
      }, 200);

      return () => clearInterval(timer);
    }
  }, [isThinking]);

  const startThinking = () => {
    setIsThinking(true);
    setHasStarted(true);
    setThinkingState("thinking");
    setDuration(0);
    setAnimatedContent([]);
  };

  return (
    <div className="container px-6 py-4">
      <div className="mx-auto">
        <Breadcrumb className="mb-6">
          <BreadcrumbList>
            <BreadcrumbItem>
              <BreadcrumbLink href="/components">Components</BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
            <BreadcrumbItem>
              <BreadcrumbPage>Chain of Thought</BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>
        <div className="flex items-center justify-between mb-8">
          <h1 className="text-2xl font-bold">Chain of Thought</h1>
          <Button onClick={startThinking} disabled={isThinking}>
            Start Demo Thinking
          </Button>
        </div>
        <div className="mx-12 mt-12">
          <Thinking
            state={thinkingState}
            duration={duration}
            animatedContent={animatedContent}
          />
        </div>
      </div>
    </div>
  );
}
