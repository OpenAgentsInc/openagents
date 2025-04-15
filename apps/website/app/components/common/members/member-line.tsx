import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { statusUserColors, User } from '@/mock-data/users';
import { format } from 'date-fns';
import { TeamsTooltip } from './teams-tooltip';

interface MemberLineProps {
   user: User;
}

export default function MemberLine({ user }: MemberLineProps) {
   return (
      <div className="w-full flex items-center py-3 px-6 border-b hover:bg-sidebar/50 border-muted-foreground/5 text-sm last:border-b-0">
         <div className="w-[70%] md:w-[60%] lg:w-[55%] flex items-center gap-2">
            <div className="relative">
               <Avatar className="size-8 shrink-0">
                  <AvatarImage src={user.avatarUrl} alt={user.name} />
                  <AvatarFallback>{user.name[0]}</AvatarFallback>
               </Avatar>
               <span
                  className="border-background absolute -end-0.5 -bottom-0.5 size-2.5 rounded-full border-2"
                  style={{ backgroundColor: statusUserColors[user.status] }}
               >
                  <span className="sr-only">{user.status}</span>
               </span>
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
            <TeamsTooltip teamIds={user.teamIds} />
         </div>
      </div>
   );
}
