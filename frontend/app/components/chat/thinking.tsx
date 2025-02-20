import { Lightbulb, Loader2, XCircle } from "lucide-react"
import { useEffect, useState } from "react"
import {
  Accordion, AccordionContent, AccordionItem, AccordionTrigger
} from "~/components/ui/accordion"
import { cn } from "~/lib/utils"

export type ThinkingState = "thinking" | "finished" | "error";

interface ThinkingProps {
  state?: ThinkingState;
  duration?: number;
  content?: string[];
}

export function Thinking({ state = "thinking", duration = 0, content = [] }: ThinkingProps) {
  const getIcon = () => {
    switch (state) {
      case "thinking":
        return <Loader2 className="text-nowrap shrink-0 h-4 w-4 animate-spin" />;
      case "finished":
        return <Lightbulb className="text-nowrap shrink-0 h-4 w-4" />;
      case "error":
        return <XCircle className="text-nowrap shrink-0 h-4 w-4 text-destructive" />;
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

  return (
    <div className={cn(
      "md:-mx-4 mb-4 relative",
      "border-2 border-toggle-border overflow-clip"
    )}>
      <Accordion type="single" collapsible className="w-full">
        <AccordionItem value="thinking" className="border-none">
          <AccordionTrigger className="group pr-6 sticky top-0 bg-background z-10">
            <div className="min-h-[2.5rem] overflow-y-clip flex flex-col justify-center text-primary relative w-full overflow-clip">
              <div className="flex h-full gap-1 w-full items-center justify-start px-5">
                <div className="flex items-center gap-2">
                  <div className="flex items-center gap-1 overflow-hidden">
                    {getIcon()}
                    <div className="flex items-baseline gap-1 overflow-hidden">
                      <span className="text-sm text-nowrap whitespace-nowrap">{getLabel()}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </AccordionTrigger>
          <AccordionContent className="text-sm text-muted-foreground px-5 pb-4">
            <div className="space-y-2 max-h-[300px] overflow-y-auto">
              {content.map((line, i) => (
                <p key={i}>{line}</p>
              ))}
            </div>
          </AccordionContent>
        </AccordionItem>
      </Accordion>
    </div>
  );
}
