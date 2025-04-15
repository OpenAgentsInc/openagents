'use client';

import { Button } from '@/components/ui/button';
import { ListFilter } from 'lucide-react';

export function Filter() {
   return (
      <Button size="xs" variant="ghost">
         <ListFilter className="size-4 mr-1" />
         Filter
      </Button>
   );
}
