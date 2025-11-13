import { ComponentPropsWithoutRef, ComponentRef, forwardRef } from "react";
import * as PopoverPrimitive from "@radix-ui/react-popover";
import { ScopedProps, usePopoverScope } from "./scope";

export namespace AssistantModalPrimitiveTrigger {
  export type Element = ComponentRef<typeof PopoverPrimitive.Trigger>;
  export type Props = ComponentPropsWithoutRef<typeof PopoverPrimitive.Trigger>;
}

export const AssistantModalPrimitiveTrigger = forwardRef<
  AssistantModalPrimitiveTrigger.Element,
  AssistantModalPrimitiveTrigger.Props
>(
  (
    {
      __scopeAssistantModal,
      ...rest
    }: ScopedProps<AssistantModalPrimitiveTrigger.Props>,
    ref,
  ) => {
    const scope = usePopoverScope(__scopeAssistantModal as any);

    return <PopoverPrimitive.Trigger {...(scope as any)} {...rest} ref={ref} />;
  },
);

AssistantModalPrimitiveTrigger.displayName = "AssistantModalPrimitive.Trigger";
