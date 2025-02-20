import { Lightbulb, Loader2, XCircle } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { Link } from "react-router"
import {
  Accordion, AccordionContent, AccordionItem, AccordionTrigger
} from "~/components/ui/accordion"
import { cn } from "~/lib/utils"

export type ThinkingState = "thinking" | "finished" | "error";

interface ThinkingProps {
  state?: ThinkingState;
  duration?: number;
  content?: string[];
  animatedContent?: { text: string; opacity: number }[];
  defaultOpen?: boolean;
  simplified?: boolean;
}

export function Thinking({
  state = "thinking",
  duration = 0,
  content = [],
  animatedContent,
  defaultOpen = false,
  simplified = false
}: ThinkingProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [accordionValue, setAccordionValue] = useState<string | undefined>(defaultOpen ? "thinking" : undefined);

  useEffect(() => {
    if (state === "thinking") {
      setAccordionValue("thinking");
    }
  }, [state]);

  useEffect(() => {
    if (scrollRef.current && contentRef.current) {
      const shouldScroll = scrollRef.current.scrollHeight > scrollRef.current.clientHeight;
      if (shouldScroll) {
        scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
      }
    }
  }, [animatedContent]);

  const getIcon = () => {
    switch (state) {
      case "thinking":
        return <Loader2 className="text-nowrap shrink-0 h-4 w-4 animate-spin mr-1" />;
      case "finished":
        return <Lightbulb className="text-nowrap shrink-0 h-4 w-4 mr-1" />;
      case "error":
        return <XCircle className="text-nowrap shrink-0 h-4 w-4 text-destructive mr-1" />;
    }
  };

  const getLabel = () => {
    switch (state) {
      case "thinking":
        return `Thinking ${duration}s`;
      case "finished":
        return `Thought for ${duration}s`;
      case "error":
        return `Error after ${duration}s`;
    }
  };

  const hasContent = animatedContent ? animatedContent.length > 0 : content.length > 0;

  if (simplified) {
    return (
      <div className="w-full">
        <Link
          to="/thinking"
          className="flex items-center gap-2 p-2 rounded-lg hover:bg-accent/50 transition-colors"
        >
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 rounded-full bg-primary animate-pulse" />
            <span>Thinking...</span>
          </div>
          <span className="text-sm text-muted-foreground">Click to view details</span>
        </Link>
      </div>
    );
  }

  return (
    <div className={cn(
      "md:-mx-4 mb-4 relative",
      "border-2 border-toggle-border overflow-clip"
    )}>
      <Accordion
        type="single"
        collapsible
        className="w-full"
        value={accordionValue}
        onValueChange={setAccordionValue}
      >
        <AccordionItem value="thinking" className="border-none">
          <AccordionTrigger className="group pr-6 sticky top-0 bg-background z-10">
            <div className="min-h-[2.5rem] overflow-y-clip flex flex-col justify-center text-primary relative w-full overflow-clip">
              <div className="flex h-full gap-1 w-full items-center justify-start px-4">
                <div className="flex items-center gap-2 pr-2">
                  <div className="flex items-center gap-1 overflow-hidden">
                    {getIcon()}
                    <div className="flex items-baseline gap-1 overflow-hidden">
                      <span className="text-sm text-nowrap whitespace-nowrap">{getLabel()}</span>
                    </div>
                  </div>
                </div>
              </div>
              <div className="absolute left-0 right-0 bottom-[-12px] h-6 bg-gradient-to-b from-background to-transparent"></div>
            </div>
          </AccordionTrigger>
          <AccordionContent className="text-sm text-muted-foreground">
            <div className="h-[180px] overflow-hidden">
              <div
                ref={scrollRef}
                className="h-full w-full overflow-y-auto px-4"
              >
                <div ref={contentRef} className="flex flex-col justify-end min-h-full">
                  <div className="space-y-1">
                    {animatedContent ? (
                      [...animatedContent].reverse().map((line, i) => (
                        <p
                          key={i}
                          className="w-full text-white text-[12px] my-0 flex items-center transition-opacity duration-300 ease-in-out"
                          style={{
                            opacity: line.opacity
                          }}
                        >
                          {line.text}
                        </p>
                      ))
                    ) : (
                      [...content].reverse().map((line, i) => (
                        <p key={i} className="w-full text-white text-[12px] my-0 flex items-center">{line}</p>
                      ))
                    )}
                  </div>
                </div>
              </div>
            </div>
          </AccordionContent>
        </AccordionItem>
      </Accordion>
    </div>
  );
}
