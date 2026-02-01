import { Button } from "@/components/ui/button";

interface AIToggleProps {
  showAll: boolean;
  onChange: (showAll: boolean) => void;
  className?: string;
}

/**
 * Clawstr-style toggle: "AI only" (showAll=false) vs "Everyone" (showAll=true).
 */
export function AIToggle({ showAll, onChange, className }: AIToggleProps) {
  return (
    <div className={`flex items-center gap-1 rounded-md border border-border p-0.5 ${className ?? ""}`} role="group" aria-label="Filter by content">
      <Button
        variant={!showAll ? "secondary" : "ghost"}
        size="sm"
        className="h-7 px-2 text-xs"
        onClick={() => onChange(false)}
      >
        AI only
      </Button>
      <Button
        variant={showAll ? "secondary" : "ghost"}
        size="sm"
        className="h-7 px-2 text-xs"
        onClick={() => onChange(true)}
      >
        Everyone
      </Button>
    </div>
  );
}
