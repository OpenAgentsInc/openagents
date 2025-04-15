import { Button } from '@/components/ui/button';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { CheckIcon, UserCircle } from 'lucide-react';
import { useId, useState } from 'react';
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { useLoaderData } from 'react-router';

interface User {
  id: string;
  name: string;
  email?: string;
  image?: string | null;
}

interface AssigneeSelectorProps {
  assigneeId: string | undefined;
  onChange: (assigneeId: string | undefined) => void;
  loaderData?: any;
}

export function AssigneeSelector({ assigneeId, onChange, loaderData: propLoaderData }: AssigneeSelectorProps) {
  const id = useId();
  const [open, setOpen] = useState<boolean>(false);
  const routeLoaderData = useLoaderData() || {};
  // Use passed loaderData prop or fall back to useLoaderData
  const loaderData = propLoaderData || routeLoaderData;
  
  // Check for users in various locations in the loader data
  let users: User[] = [];
  
  if (loaderData.options && Array.isArray(loaderData.options.users)) {
    users = loaderData.options.users;
  } else if (Array.isArray(loaderData.users)) {
    users = loaderData.users;
  }
  
  console.log('Using loader data from props:', !!propLoaderData);
  console.log('Found users:', users?.length || 0);

  const handleAssigneeChange = (userId: string | undefined) => {
    onChange(userId);
    setOpen(false);
  };

  const selectedUser = users.find(user => user.id === assigneeId);

  // If no users are available, show a disabled button
  if (!users || users.length === 0) {
    return (
      <Button 
        variant="outline" 
        size="sm" 
        className="h-8 border-dashed flex items-center gap-1.5 bg-muted/50" 
        disabled
      >
        <UserCircle className="size-4" />
        <span className="text-muted-foreground">No users available</span>
      </Button>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          className="flex items-center gap-1.5"
          size="sm"
          variant="outline"
          role="combobox"
          aria-expanded={open}
        >
          {selectedUser ? (
            <>
              <Avatar className="size-5">
                <AvatarImage src={selectedUser.image || ''} alt={selectedUser.name} />
                <AvatarFallback>{selectedUser.name.charAt(0)}</AvatarFallback>
              </Avatar>
              <span>{selectedUser.name}</span>
            </>
          ) : (
            <>
              <UserCircle className="size-4" />
              <span>Unassigned</span>
            </>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="w-full min-w-[var(--radix-popper-anchor-width)] p-0"
        align="start"
      >
        <Command>
          <CommandInput placeholder="Search users..." />
          <CommandList>
            <CommandEmpty>No users found.</CommandEmpty>
            <CommandGroup>
              <CommandItem
                value="unassigned"
                onSelect={() => handleAssigneeChange(undefined)}
                className="flex items-center justify-between"
              >
                <div className="flex items-center gap-2">
                  <UserCircle className="size-4" />
                  Unassigned
                </div>
                {assigneeId === undefined && <CheckIcon size={16} className="ml-auto" />}
              </CommandItem>
              {users.map((user) => (
                <CommandItem
                  key={user.id}
                  value={user.name}
                  onSelect={() => handleAssigneeChange(user.id)}
                  className="flex items-center justify-between"
                >
                  <div className="flex items-center gap-2">
                    <Avatar className="size-5">
                      <AvatarImage src={user.image || ''} alt={user.name} />
                      <AvatarFallback>{user.name.charAt(0)}</AvatarFallback>
                    </Avatar>
                    <span>{user.name}</span>
                  </div>
                  {assigneeId === user.id && <CheckIcon size={16} className="ml-auto" />}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}