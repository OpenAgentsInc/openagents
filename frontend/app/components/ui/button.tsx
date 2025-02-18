import { forwardRef } from "react"
import { cn } from "../../lib/utils"

import type { ButtonHTMLAttributes } from "react";
interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  size?: "default" | "lg";
}

const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, size = "default", children, ...props }, ref) => {
    return (
      <button
        className={cn(
          "relative bg-black hover:bg-zinc-900 text-white inline-flex items-center justify-center gap-2 whitespace-nowrap select-none text-center align-middle no-underline outline-none border border-white shadow-nav hover:shadow-nav-hover active:shadow-nav-active transition-all duration-nav ease-nav group touch-manipulation",
          size === "lg" ? "px-4 py-3 text-base" : "px-3 py-2 text-sm",
          className
        )}
        ref={ref}
        {...props}
      >
        {children}
      </button>
    );
  }
);

Button.displayName = "Button";

export { Button };
