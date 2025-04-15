import { Team } from '@/mock-data/teams';
import { MembersTooltip } from './members-tooltip';
import { ProjectsTooltip } from './projects-tooltip';
import { Check } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface TeamLineProps {
   team: Team;
}

export default function TeamLine({ team }: TeamLineProps) {
   return (
      <div className="w-full flex items-center py-3 px-6 border-b hover:bg-sidebar/50 border-muted-foreground/5 text-sm">
         <div className="w-[70%] sm:w-[50%] md:w-[45%] lg:w-[40%] flex items-center gap-2">
            <div className="relative">
               <div className="inline-flex size-6 bg-muted/50 items-center justify-center rounded shrink-0">
                  <div className="text-sm">{team.icon}</div>
               </div>
            </div>
            <div className="flex flex-col items-start overflow-hidden">
               <span className="font-medium truncate w-full">{team.name}</span>
            </div>
         </div>
         <div className="hidden sm:block sm:w-[20%] md:w-[15%] text-xs text-muted-foreground">
            {team.joined && (
               <Button variant="secondary" size="xxs" className="text-xs">
                  <Check className="size-4" />
                  Joined
               </Button>
            )}
         </div>
         <div className="hidden sm:block sm:w-[20%] md:w-[15%] text-xs text-muted-foreground">
            {team.id}
         </div>
         <div className="w-[30%] sm:w-[20%] md:w-[15%] flex">
            {team.members.length > 0 && <MembersTooltip members={team.members} />}
         </div>
         <div className="hidden sm:flex sm:w-[20%] md:w-[15%] text-xs text-muted-foreground">
            {team.projects.length > 0 && <ProjectsTooltip projects={team.projects} />}
         </div>
      </div>
   );
}
