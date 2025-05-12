import { cn } from "@/lib/utils";

interface SpinnerProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: "sm" | "md" | "lg";
}

export function Spinner({ className, size = "md", ...props }: SpinnerProps) {
  const sizeClass = 
    size === "sm" ? "h-4 w-4 border-2" :
    size === "lg" ? "h-8 w-8 border-3" :
    "h-6 w-6 border-2";
  
  return (
    <div 
      className={cn(
        "animate-spin rounded-full border-solid border-t-transparent border-primary", 
        sizeClass,
        className
      )} 
      role="status"
      aria-label="Loading"
      {...props}
    >
      <span className="sr-only">Loading...</span>
    </div>
  );
}