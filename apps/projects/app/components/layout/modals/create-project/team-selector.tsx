import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useState } from 'react';
import { Users } from 'lucide-react';
import { useLoaderData } from 'react-router';

interface TeamData {
  id: string;
  name: string;
  key: string;
  icon: string | null;
  color: string | null;
}

interface TeamSelectorProps {
  selectedTeamIds: string[];
  onChange: (teamIds: string[]) => void;
}

interface LoaderData {
  options: {
    statuses: any[];
    users: any[];
    teams: TeamData[];
  };
}

export function TeamSelector({ selectedTeamIds, onChange }: TeamSelectorProps) {
  const { options } = useLoaderData() as LoaderData;
  const teams = options.teams || [];

  const handleTeamToggle = (teamId: string) => {
    if (selectedTeamIds.includes(teamId)) {
      onChange(selectedTeamIds.filter(id => id !== teamId));
    } else {
      onChange([...selectedTeamIds, teamId]);
    }
  };

  if (teams.length === 0) {
    return (
      <Button size="sm" variant="outline" className="gap-1.5" disabled>
        No teams available
      </Button>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="sm" variant="outline" className="gap-1.5">
          <Users className="size-4" />
          <span>
            {selectedTeamIds.length > 0 
              ? `Teams (${selectedTeamIds.length})` 
              : 'Teams'}
          </span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-56">
        {teams.map((team) => (
          <DropdownMenuCheckboxItem
            key={team.id}
            checked={selectedTeamIds.includes(team.id)}
            onCheckedChange={() => handleTeamToggle(team.id)}
          >
            <div className="flex items-center gap-2">
              <div 
                className="inline-flex size-5 items-center justify-center rounded shrink-0"
                style={{ backgroundColor: team.color || '#6366F1' }}
              >
                <span className="text-xs">{team.icon || team.key.substring(0, 1)}</span>
              </div>
              <span>{team.name}</span>
            </div>
          </DropdownMenuCheckboxItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}