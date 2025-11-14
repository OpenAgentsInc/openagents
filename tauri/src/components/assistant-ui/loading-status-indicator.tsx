import { useEffect, useState } from "react";
import { cn } from "@openagentsinc/ui";

export function LoadingStatusIndicator() {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    // Check loading state from global window object
    const checkLoadingState = () => {
      const loadingState = (window as any).__loadingState;
      if (loadingState?.isWaitingForFirstResponse && loadingState?.sessionStartTime) {
        setIsVisible(true);
        const elapsed = Math.floor((Date.now() - loadingState.sessionStartTime) / 1000);
        setElapsedSeconds(elapsed);
      } else {
        setIsVisible(false);
      }
    };

    // Check immediately
    checkLoadingState();

    // Update every second
    const interval = setInterval(checkLoadingState, 1000);

    // Listen for state changes
    const handleStateChange = () => {
      checkLoadingState();
    };
    window.addEventListener('loadingStateChanged', handleStateChange);

    return () => {
      clearInterval(interval);
      window.removeEventListener('loadingStateChanged', handleStateChange);
    };
  }, []);

  if (!isVisible) return null;

  return (
    <div className={cn(
      "flex items-center gap-2 text-sm text-muted-foreground py-2 px-3",
      "font-mono"
    )}>
      <div className="flex items-center gap-1">
        <span>Working</span>
        <span className="inline-flex gap-0.5">
          <span className="animate-pulse">.</span>
          <span className="animate-pulse" style={{ animationDelay: "0.2s" }}>.</span>
          <span className="animate-pulse" style={{ animationDelay: "0.4s" }}>.</span>
        </span>
      </div>
      <span className="text-muted-foreground/70">{elapsedSeconds}s</span>
    </div>
  );
}
