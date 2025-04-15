import { Button } from '@/components/ui/button';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
} from '@/components/ui/command';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Users } from 'lucide-react';
import { useState, useEffect } from 'react';
import { useLoaderData } from 'react-router';
import { cn } from '@/lib/utils';
import { ScrollArea } from '@/components/ui/scroll-area';

// Define structure of loader data that includes teams
interface LoaderData {
  teams?: Array<{
    id: string;
    name: string;
    icon?: string;
    color?: string;
    key?: string;
  }>;
}

interface TeamSelectorProps {
  teamId: string | null;
  onChange: (teamId: string) => void;
}

export function TeamSelector({ teamId, onChange }: TeamSelectorProps) {
  const [open, setOpen] = useState(false);
  const loaderData = useLoaderData() || {};
  
  // Check for teams in various locations in the loader data
  let teams: any[] = [];
  
  if (Array.isArray(loaderData.teams)) {
    teams = loaderData.teams;
  } else if (loaderData.options && Array.isArray(loaderData.options.teams)) {
    teams = loaderData.options.teams;
  }
  
  console.log('Teams available:', teams.length, teams);
  
  // Set default team if none is selected
  useEffect(() => {
    if (!teamId && teams.length > 0) {
      onChange(teams[0].id);
    }
  }, [teamId, teams, onChange]);

  const selectedTeam = teams.find((team) => team.id === teamId);

  // If no teams are available, show a disabled button with clear message
  if (!teams || teams.length === 0) {
    return (
      <Button variant="outline" size="sm" className="h-8 border-dashed bg-muted/50" disabled>
        <Users className="mr-2 size-4" />
        <span className="text-muted-foreground">No teams available</span>
      </Button>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className="h-8 border-dashed">
          {selectedTeam ? (
            <>
              <div className="w-4 h-4 mr-2 flex items-center justify-center">
                {selectedTeam.icon ? (
                  <span>{selectedTeam.icon}</span>
                ) : (
                  <Users className="size-4" />
                )}
              </div>
              {selectedTeam.name}
            </>
          ) : (
            <>
              <Users className="mr-2 size-4" />
              <span>Select Team</span>
            </>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="p-0" side="bottom" align="start">
        <Command>
          <CommandInput placeholder="Search teams..." />
          <CommandEmpty>No teams found.</CommandEmpty>
          <CommandGroup>
            <ScrollArea className="h-72">
              {teams.map((team) => (
                <CommandItem
                  key={team.id}
                  value={team.name}
                  onSelect={() => {
                    onChange(team.id);
                    setOpen(false);
                  }}
                  className={cn(
                    'flex items-center gap-2',
                    teamId === team.id && 'bg-accent'
                  )}
                >
                  <div 
                    className="w-4 h-4 flex items-center justify-center"
                    style={{ color: team.color || 'currentColor' }}
                  >
                    {team.icon ? (
                      <span>{team.icon}</span>
                    ) : (
                      <Users className="size-4" />
                    )}
                  </div>
                  <span>{team.name}</span>
                  {team.key && (
                    <span className="ml-auto text-xs text-muted-foreground">
                      {team.key}
                    </span>
                  )}
                </CommandItem>
              ))}
            </ScrollArea>
          </CommandGroup>
        </Command>
      </PopoverContent>
    </Popover>
  );
}