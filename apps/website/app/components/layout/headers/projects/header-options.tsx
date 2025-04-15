'use client';

import { Button } from '@/components/ui/button';
import { ListFilter, SlidersHorizontal } from 'lucide-react';

export default function HeaderOptions() {
   return (
      <div className="w-full flex justify-between items-center border-b py-1.5 px-6 h-10">
         <Button size="xs" variant="ghost">
            <ListFilter className="size-4" />
            <span className="hidden sm:inline ml-1">Filter</span>
         </Button>
         <Button className="relative" size="xs" variant="secondary">
            <SlidersHorizontal className="size-4" />
            <span className="hidden sm:inline ml-1">Display</span>
         </Button>
      </div>
   );
}
