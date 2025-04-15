import { HeaderNav } from './header-nav';
import { HeaderOptions } from './header-options';
import { Button } from '@/components/ui/button';
import { Plus } from 'lucide-react';
import { useCreateIssueStore } from '@/store/create-issue-store';

export function HeaderIssues() {
  const { openModal } = useCreateIssueStore();

  return (
    <div className="w-full flex flex-col items-center">
      <div className="container mx-auto py-4 px-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">Issues</h1>
        <Button onClick={openModal} size="sm" className="gap-1.5">
          <Plus className="size-4" />
          New Issue
        </Button>
      </div>
      <HeaderNav />
      <HeaderOptions />
    </div>
  );
}
