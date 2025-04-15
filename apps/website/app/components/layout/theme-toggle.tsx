'use client';

import * as React from 'react';
import { Moon, Sun, Laptop, Leaf } from 'lucide-react';
import { useTheme } from 'next-themes';
import { Button } from '@/components/ui/button';
import {
   DropdownMenu,
   DropdownMenuContent,
   DropdownMenuItem,
   DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';

export function ThemeToggle() {
   const { theme, setTheme } = useTheme();

   // To avoid a hydration error caused by mismatched server/client rendering,
   // we wait for the component to mount before using `theme` from `next-themes`,
   // since it relies on localStorage and is not available during SSR.s
   const [mounted, setMounted] = React.useState(false);

   React.useEffect(() => {
      setMounted(true);
   }, []);

   if (!mounted) return null;

   return (
      <DropdownMenu>
         <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="h-8 w-8 shrink-0">
               {theme === 'light' ? (
                  <Sun className="h-4 w-4" />
               ) : theme === 'dark' ? (
                  <Moon className="h-4 w-4" />
               ) : theme === 'ghibli' ? (
                  <Leaf className="h-4 w-4" />
               ) : (
                  <Laptop className="h-4 w-4" />
               )}
               <span className="sr-only">Toggle theme</span>
            </Button>
         </DropdownMenuTrigger>
         <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => setTheme('light')}>
               <Sun className="mr-2 h-4 w-4" />
               <span>Light</span>
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTheme('dark')}>
               <Moon className="mr-2 h-4 w-4" />
               <span>Dark</span>
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTheme('ghibli')}>
               <Leaf className="mr-2 h-4 w-4" />
               <span>Ghibli</span>
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTheme('system')}>
               <Laptop className="mr-2 h-4 w-4" />
               <span>System</span>
            </DropdownMenuItem>
         </DropdownMenuContent>
      </DropdownMenu>
   );
}
