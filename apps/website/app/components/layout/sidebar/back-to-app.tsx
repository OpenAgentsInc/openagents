import { Button } from '@/components/ui/button';
import { ChevronLeft } from 'lucide-react';
import { Link } from 'react-router';
import { ThemeToggle } from '@/components/layout/theme-toggle';

export function BackToApp() {
  return (
    <div className="w-full flex items-center justify-between gap-2">
      <Button className="w-fit" size="xs" variant="outline" asChild>
        <Link to="/lndev-ui/team/CORE/all">
          <ChevronLeft className="size-4" />
          Back to app
        </Link>
      </Button>
      <ThemeToggle />
    </div>
  );
}
