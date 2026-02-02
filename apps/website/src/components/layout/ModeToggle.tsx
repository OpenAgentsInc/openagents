import * as React from "react";
import { Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";

/** Effective theme is system until user clicks; then we only store "light" | "dark". */
function getEffectiveDark(): boolean {
  if (typeof window === "undefined") return true;
  const stored = localStorage.getItem("theme");
  if (stored === "light") return false;
  if (stored === "dark") return true;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function ModeToggle() {
  const handleToggle = React.useCallback(() => {
    const nextDark = !getEffectiveDark();
    document.documentElement.classList[nextDark ? "add" : "remove"]("dark");
    document.documentElement.style.colorScheme = nextDark ? "dark" : "light";
    localStorage.setItem("theme", nextDark ? "dark" : "light");
  }, []);

  return (
    <Button variant="outline" size="icon" onClick={handleToggle} type="button">
      <Sun className="h-[1.2rem] w-[1.2rem] scale-100 rotate-0 transition-all dark:scale-0 dark:-rotate-90" />
      <Moon className="absolute h-[1.2rem] w-[1.2rem] scale-0 rotate-90 transition-all dark:scale-100 dark:rotate-0" />
      <span className="sr-only">Toggle light/dark</span>
    </Button>
  );
}
