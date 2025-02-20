import { useEffect, useState } from "react"
import { Link } from "react-router"
import { Thinking } from "~/components/chat/thinking"
import {
  Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage,
  BreadcrumbSeparator
} from "~/components/ui/breadcrumb"
import { Button } from "~/components/ui/button"

// Single long paragraph of text
const DEMO_TEXT = Array(10).fill([
  "quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit, sed quia non numquam",
  "fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt. Neque porro",
  "beatae vitae dicta sunt explicabo. Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut",
  "doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto",
  "mollit anim id est laborum. Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium",
  "fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt",
  "ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu"
]).flat();

export default function ThinkingPage() {
  const [isThinking, setIsThinking] = useState(false);
  const [hasStarted, setHasStarted] = useState(false);
  const [thinkingState, setThinkingState] = useState<"thinking" | "finished" | "error">("finished");
  const [duration, setDuration] = useState(0);
  const [animatedContent, setAnimatedContent] = useState<Array<{ text: string; opacity: number }>>([]);

  const allLines = DEMO_TEXT;  // No need to split, using pre-split lines

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
        setAnimatedContent(prev => [
          { text: allLines[currentLineIndex], opacity: 0 },
          ...prev
        ]);

        // Fade in the latest line
        setTimeout(() => {
          setAnimatedContent(prev =>
            prev.map((line, i) =>
              i === 0 ? { ...line, opacity: 1 } : line
            )
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
        <Thinking
          state={thinkingState}
          duration={duration}
          animatedContent={animatedContent}
        />
      </div>
    </div>
  );
}
