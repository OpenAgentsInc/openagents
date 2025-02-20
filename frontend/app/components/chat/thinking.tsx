import {
  Accordion, AccordionContent, AccordionItem, AccordionTrigger
} from "~/components/ui/accordion"

export function Thinking() {
  return (
    <Accordion type="single" collapsible className="w-full">
      <AccordionItem value="thinking">
        <AccordionTrigger className="text-muted-foreground">
          Thinking...
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-2 text-sm text-muted-foreground">
            <p>1. Analyzing the request and breaking it down into steps</p>
            <p>2. Searching relevant documentation and context</p>
            <p>3. Formulating a response based on gathered information</p>
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  );
}
