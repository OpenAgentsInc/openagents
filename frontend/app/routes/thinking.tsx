import { useEffect, useState } from "react"
import { Thinking } from "~/components/chat/thinking"
import { Button } from "~/components/ui/button"

// Single long paragraph of text
const DEMO_TEXT = `Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo. Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt. Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit, sed quia non numquam eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem.`;

export default function ThinkingPage() {
  const [isThinking, setIsThinking] = useState(false);
  const [hasStarted, setHasStarted] = useState(false);
  const [thinkingState, setThinkingState] = useState<"thinking" | "finished" | "error">("finished");
  const [duration, setDuration] = useState(0);
  const [animatedContent, setAnimatedContent] = useState<Array<{ text: string; opacity: number }>>([]);

  // Pre-split into visual lines (roughly 60-70 chars per line)
  const allLines = DEMO_TEXT.split(' ').reduce((acc: string[], word) => {
    const currentLine = acc[acc.length - 1] || '';
    if (!currentLine || (currentLine + ' ' + word).length > 105) {
      acc.push(word);
    } else {
      acc[acc.length - 1] = currentLine + ' ' + word;
    }
    return acc;
  }, []);

  useEffect(() => {
    if (isThinking) {
      let startTime = Date.now();
      let currentLineIndex = 0;

      // Start with all lines at 0 opacity
      setAnimatedContent(allLines.map(text => ({ text, opacity: 0 })));

      const timer = setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTime) / 1000);
        setDuration(elapsed);

        if (elapsed >= 10 || currentLineIndex >= allLines.length) {
          clearInterval(timer);
          setIsThinking(false);
          setThinkingState("finished");
          setAnimatedContent(prev => prev.map(line => ({ ...line, opacity: 1 })));
          return;
        }

        // Fade in the next line
        setAnimatedContent(prev =>
          prev.map((line, i) => ({
            ...line,
            opacity: i <= currentLineIndex ? 1 : 0
          }))
        );

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
    setAnimatedContent(allLines.map(text => ({ text, opacity: 0 })));
  };

  return (
    <div className="p-4 bg-background">
      <div className="w-full max-w-3xl mx-auto">
        <div className="flex items-center justify-between mb-8">
          <h1 className="text-2xl font-bold">Thinking Process</h1>
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
