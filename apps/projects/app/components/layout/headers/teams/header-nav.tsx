import { Button } from '@/components/ui/button';
import { SidebarTrigger } from '@/components/ui/sidebar';
import { Plus } from 'lucide-react';
import { useLoaderData } from 'react-router';
import { useCreateTeamStore } from '@/store/create-team-store';

interface LoaderData {
  teams: any[];
  user: any;
}

export function HeaderNav() {
  const { teams = [] } = useLoaderData() as LoaderData;
  const { openModal } = useCreateTeamStore();

  return (
    <div className="w-full flex justify-between items-center border-b py-1.5 px-6 h-10">
      <div className="flex items-center gap-2">
        <SidebarTrigger className="" />
        <div className="flex items-center gap-1">
          <span className="text-sm font-medium">Teams</span>
          <span className="text-xs bg-accent rounded-md px-1.5 py-1">{teams.length}</span>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <Button 
          className="relative" 
          size="xs" 
          variant="secondary"
          onClick={openModal}
        >
          <Plus className="size-4" />
          Add team
        </Button>
      </div>
    </div>
  );
}

export default HeaderNav;