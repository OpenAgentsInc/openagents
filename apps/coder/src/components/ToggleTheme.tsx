import { Moon as LucideMoon } from "lucide-react";
import React from "react";
import { Button } from "@/components/ui/button";
import { toggleTheme } from "@/helpers/theme_helpers";
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

export default function ToggleTheme() {
  return (
    <Button onClick={toggleTheme} size="icon">
      <Moon size={16} />
    </Button>
  );
}
