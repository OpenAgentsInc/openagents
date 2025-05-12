import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { ContactRound } from 'lucide-react';

interface TeamInfo {
  teamId: string;
  teamName: string;
  teamKey: string;
  teamIcon: string;
  owner: number;
}

interface TeamsTooltipProps {
  teamIds: string[];
  teams?: TeamInfo[];
}

export function TeamsTooltip({ teamIds, teams = [] }: TeamsTooltipProps) {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger className="flex items-center gap-0.5 truncate">
        <ContactRound className="text-muted-foreground size-4 mr-1 shrink-0" />
        {teamIds.slice(0, 2).map((teamId, index) => (
          <span key={teamId} className="mt-0.5">
            {teamId}
            {index < Math.min(teamIds.length, 2) - 1 && ', '}
          </span>
        ))}
        {teamIds.length > 2 && <span className="mt-0.5">+ {teamIds.length - 2}</span>}
      </TooltipTrigger>
      <TooltipContent className="p-2">
        <div className="flex flex-col gap-1">
          {teams.map((team) => (
            <div key={team.teamId} className="text-xs flex items-center gap-2">
              <div className="inline-flex size-6 bg-muted/50 items-center justify-center rounded shrink-0">
                <div className="text-sm">{team.teamIcon || '👥'}</div>
              </div>
              <span className="font-medium">{team.teamName}</span>
              {team.owner === 1 && (
                <span className="text-xs bg-primary/10 text-primary px-1.5 py-0.5 rounded-full">
                  Owner
                </span>
              )}
            </div>
          ))}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}
