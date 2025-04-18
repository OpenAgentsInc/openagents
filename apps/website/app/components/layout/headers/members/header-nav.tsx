
import { Button } from '@/components/ui/button';
import { useLoaderData } from 'react-router';

export default function HeaderNav() {
  const { users = [] } = useLoaderData<{ users: any[] }>();

  return (
    <div className="!select-none w-full flex justify-between items-center border-b py-1.5 px-6 h-10">
      <div className="flex items-center gap-2">
        <div className="flex items-center gap-1">
          <span className="text-sm font-medium">Members</span>
          <span className="text-xs bg-accent rounded-md px-1.5 py-1">{users.length}</span>
        </div>
      </div>
      <div className="flex items-center gap-2">
        {/* <Button className="relative" size="xs" variant="secondary">
          <Plus className="size-4" />
          Invite
        </Button> */}
      </div>
    </div>
  );
}
