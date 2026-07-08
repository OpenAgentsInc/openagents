import * as React from "react";

import { cn } from "@/lib/utils";

function Accordion({
  className,
  type: _type,
  collapsible: _collapsible,
  ...props
}: React.ComponentProps<"div"> & {
  type?: "single" | "multiple";
  collapsible?: boolean;
}) {
  return <div data-slot="accordion" className={className} {...props} />;
}

function AccordionItem({
  className,
  value: _value,
  ...props
}: React.ComponentProps<"details"> & { value?: string }) {
  return (
    <details
      data-slot="accordion-item"
      className={cn("group border-border border-b dark:border-border/15", className)}
      {...props}
    />
  );
}

function AccordionTrigger({
  className,
  children,
  ...props
}: React.ComponentProps<"summary">) {
  return (
    <summary
      data-slot="accordion-trigger"
      className={cn(
        "text-md flex cursor-pointer list-none items-center justify-between py-4 text-left font-medium transition-all outline-none hover:underline focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 [&::-webkit-details-marker]:hidden",
        className,
      )}
      {...props}
    >
      {children}
      <span
        aria-hidden="true"
        className="pointer-events-none text-lg leading-none text-muted-foreground transition-transform duration-200 group-open:rotate-180"
      >
        ⌄
      </span>
    </summary>
  );
}

function AccordionContent({
  className,
  children,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="accordion-content"
      className={cn("overflow-hidden pt-0 pb-4 text-sm", className)}
      {...props}
    >
      {children}
    </div>
  );
}

export { Accordion, AccordionContent, AccordionItem, AccordionTrigger };
