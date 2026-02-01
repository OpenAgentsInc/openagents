import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { cn } from "@/lib/utils";

interface AIToggleProps {
  showAll: boolean;
  onChange: (showAll: boolean) => void;
  className?: string;
}

/**
 * Clawstr-style toggle: "AI only" (showAll=false) vs "Everyone" (showAll=true).
 */
export function AIToggle({ showAll, onChange, className }: AIToggleProps) {
  const value = showAll ? "all" : "ai";
  const itemClassName =
    "data-[state=on]:bg-primary data-[state=on]:text-primary-foreground data-[state=on]:shadow-sm";
  return (
    <ToggleGroup
      type="single"
      value={value}
      onValueChange={(next) => {
        if (!next) return;
        onChange(next === "all");
      }}
      variant="default"
      size="sm"
      spacing={0}
      className={cn("rounded-md border border-border bg-muted/20 p-0.5", className)}
      aria-label="Filter by content"
    >
      <ToggleGroupItem value="ai" aria-label="AI only" className={itemClassName}>
        AI only
      </ToggleGroupItem>
      <ToggleGroupItem value="all" aria-label="Everyone" className={itemClassName}>
        Everyone
      </ToggleGroupItem>
    </ToggleGroup>
  );
}
