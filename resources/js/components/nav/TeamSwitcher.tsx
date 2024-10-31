import * as React from "react"
import { Button } from "@/components/ui/button"
import {
  Command, CommandEmpty, CommandGroup, CommandItem, CommandList,
  CommandSeparator
} from "@/components/ui/command"
import {
  Popover, PopoverContent, PopoverTrigger
} from "@/components/ui/popover"
import { cn } from "@/lib/utils"
import { usePage, router } from "@inertiajs/react"
import {
  CaretSortIcon, CheckIcon, PlusCircledIcon
} from "@radix-ui/react-icons"

type Team = {
  id: number | null
  name: string
}

type PopoverTriggerProps = React.ComponentPropsWithoutRef<typeof PopoverTrigger>

interface TeamSwitcherProps extends PopoverTriggerProps { }

export default function TeamSwitcher({ className }: TeamSwitcherProps) {
  const { auth } = usePage().props as any
  const [open, setOpen] = React.useState(false)

  const teams = auth.teams || []
  const currentTeam = auth.current_team

  const groups = [
    {
      label: "Personal Account",
      teams: [
        {
          id: null,
          name: auth.user?.name || "Personal",
        },
      ],
    },
    {
      label: "Teams",
      teams: teams,
    },
  ]

  const handleTeamSelect = (team: Team) => {
    setOpen(false)
    router.post(route('teams.switch'), { team_id: team.id }, {
      preserveState: true,
      preserveScroll: true,
    })
  }

  const handleCreateTeam = () => {
    router.get(route('teams.create'))
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          aria-label="Select a team"
          className={cn("w-[200px] justify-between", className)}
        >
          <span className="truncate">
            {currentTeam?.name || auth.user?.name || 'Select Team'}
          </span>
          <CaretSortIcon className="ml-auto h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0" style={{ minWidth: '200px', maxWidth: '250px' }}>
        <Command>
          <CommandList>
            {groups.map((group) => (
              <CommandGroup key={group.label} heading={group.label}>
                {group.teams.length > 0 ? (
                  group.teams.map((team) => (
                    <CommandItem
                      key={team.id ?? 'personal'}
                      onSelect={() => handleTeamSelect(team)}
                      className="text-sm"
                    >
                      <span className="truncate">{team.name}</span>
                      <CheckIcon
                        className={cn(
                          "ml-auto h-4 w-4 flex-shrink-0",
                          (currentTeam?.id === team.id || (!currentTeam && !team.id))
                            ? "opacity-100"
                            : "opacity-0"
                        )}
                      />
                    </CommandItem>
                  ))
                ) : (
                  <CommandItem className="text-sm">No teams</CommandItem>
                )}
              </CommandGroup>
            ))}
          </CommandList>
          <CommandSeparator />
          <CommandList>
            <CommandGroup>
              <CommandItem onSelect={handleCreateTeam} className="text-sm">
                <PlusCircledIcon className="mr-2 h-4 w-4" />
                Create Team
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  )
}