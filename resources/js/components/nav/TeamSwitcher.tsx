import * as React from "react"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Button } from "@/components/ui/button"
import {
  Command, CommandEmpty, CommandGroup, CommandItem, CommandList,
  CommandSeparator
} from "@/components/ui/command"
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader,
  DialogTitle, DialogTrigger
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Popover, PopoverContent, PopoverTrigger
} from "@/components/ui/popover"
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue
} from "@/components/ui/select"
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
  const [showNewTeamDialog, setShowNewTeamDialog] = React.useState(false)
  const [selectedTeam, setSelectedTeam] = React.useState<Team | null>(
    auth.current_team || { id: null, name: auth.user?.name || "Personal" }
  )

  const teams = auth.teams || []

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
    setSelectedTeam(team)
    setOpen(false)
    router.post('/switch-team', { team_id: team.id }, {
      preserveState: true,
      preserveScroll: true,
    })
  }

  const handleCreateTeam = () => {
    router.get('/teams/create')
  }

  return (
    <Dialog open={showNewTeamDialog} onOpenChange={setShowNewTeamDialog}>
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
              {selectedTeam?.name || 'Select Team'}
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
                            selectedTeam?.id === team.id
                              ? "opacity-100"
                              : "opacity-0"
                          )}
                        />
                      </CommandItem>
                    ))
                  ) : (
                    <CommandItem className="text-sm">None</CommandItem>
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
    </Dialog>
  )
}