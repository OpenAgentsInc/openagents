import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { format } from 'date-fns';
import { TeamsTooltip } from './teams-tooltip';

// Database user enhanced with team information
interface DBUser {
  id: string;
  name: string;
  email: string;
  image: string | null;
  teams: any[];
  teamIds: string[];
  joinedDate: string;
  role: string;
}

interface MemberLineProps {
  user: DBUser;
}

export default function MemberLine({ user }: MemberLineProps) {
  return (
    <div className="w-full flex items-center py-3 px-6 border-b hover:bg-sidebar/50 border-muted-foreground/5 text-sm last:border-b-0">
      <div className="w-[70%] md:w-[60%] lg:w-[55%] flex items-center gap-2">
        <div className="relative">
          <Avatar className="size-8 shrink-0">
            <AvatarImage src={user.image || ''} alt={user.name} />
            <AvatarFallback>{user.name.charAt(0)}</AvatarFallback>
          </Avatar>
        </div>
        <div className="flex flex-col items-start overflow-hidden">
          <span className="font-medium truncate w-full">{user.name}</span>
          <span className="text-xs text-muted-foreground truncate w-full">{user.email}</span>
        </div>
      </div>
      <div className="w-[30%] md:w-[20%] lg:w-[15%] text-xs text-muted-foreground">
        {user.role}
      </div>
      <div className="hidden lg:block w-[15%] text-xs text-muted-foreground">
        {format(new Date(user.joinedDate), 'MMM yyyy')}
      </div>
      <div className="w-[30%] hidden md:flex md:w-[20%] lg:w-[15%] text-xs text-muted-foreground">
        {user.teamIds && user.teamIds.length > 0 ? (
          <TeamsTooltip teamIds={user.teamIds} teams={user.teams} />
        ) : (
          <span className="text-xs text-muted-foreground">No teams</span>
        )}
      </div>
    </div>
  );
}
