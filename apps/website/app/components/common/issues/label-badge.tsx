import { Badge } from '@/components/ui/badge';
import { LabelInterface } from '@/mock-data/labels';

export function LabelBadge({ label }: { label: LabelInterface[] }) {
   return (
      <>
         {label.map((l) => (
            <Badge
               key={l.id}
               variant="outline"
               className="gap-1.5 rounded-full text-muted-foreground bg-background"
            >
               <span
                  className="size-1.5 rounded-full"
                  style={{ backgroundColor: l.color }}
                  aria-hidden="true"
               ></span>
               {l.name}
            </Badge>
         ))}
      </>
   );
}
