import { Button } from "@/components/button/Button";
import type { ButtonProps } from "@/components/button/Button";
import { cn } from "@/lib/utils";
import { ArrowsClockwiseIcon } from "@phosphor-icons/react";

export const RefreshButton = ({ toggled, ...props }: ButtonProps) => (
  <Button shape="square" {...(toggled !== undefined ? { toggled } : {})} {...props}>
    <ArrowsClockwiseIcon
      className={cn({
        "animate-refresh": toggled,
        "size-4.5": props.size === "base",
        "size-4": props.size === "sm",
        "size-5": props.size === "lg"
      })}
    />
  </Button>
);
