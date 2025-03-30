import { Moon as LucideMoon, Sun as LucideSun } from "lucide-react";
import React, { useEffect } from "react";
import { Button } from "@/components/ui/button";
import { useDarkMode } from "@/hooks/use-dark-mode";
import { react19 } from "@openagents/core";

// Define interface for the icon props
interface IconProps {
  size?: number;
  color?: string;
  className?: string;
  [key: string]: any;
}

// Make Lucide icons compatible with React 19
const Moon = react19.icon<IconProps>(LucideMoon);
const Sun = react19.icon<IconProps>(LucideSun);

export default function ToggleTheme() {
  const { isDark, toggleDarkMode } = useDarkMode();

  // This logs the current class status to help with debugging
  useEffect(() => {
    console.log("ToggleTheme: Dark mode is", isDark ? "ON" : "OFF");
    console.log("HTML classes:", document.documentElement.className);
  }, [isDark]);

  return (
    <Button 
      onClick={toggleDarkMode} 
      size="icon"
      aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
      className={isDark ? "bg-primary text-background" : ""}
    >
      {isDark ? <Sun size={16} /> : <Moon size={16} />}
    </Button>
  );
}
