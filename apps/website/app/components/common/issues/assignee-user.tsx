import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useIssuesStore } from '@/store/issues-store';
import { CheckIcon, CircleUserRound, Send, UserIcon } from 'lucide-react';
import { useState, useEffect } from 'react';
import { useLoaderData, useSubmit } from 'react-router';

// Database user interface
interface User {
  id: string;
  name: string;
  email?: string;
  image?: string | null;
}

interface AssigneeUserProps {
  user: User | null;
  issueId?: string; // Make issueId an optional prop
}

export function AssigneeUser({ user, issueId }: AssigneeUserProps) {
  const [open, setOpen] = useState(false);
  const [currentAssignee, setCurrentAssignee] = useState<User | null>(user);
  const loaderData = useLoaderData<any>();
  const { updateIssueAssignee } = useIssuesStore();
  const submit = useSubmit();
  
  // Get users from loader data
  const users = loaderData?.options?.users || [];

  // Keep local state in sync with prop changes
  useEffect(() => {
    setCurrentAssignee(user);
  }, [user]);

  const handleAssigneeChange = (newAssignee: User | null) => {
    setCurrentAssignee(newAssignee);
    setOpen(false);
    
    // Update the issue in the store if we have an issueId
    if (issueId) {
      // Update local state first for immediate UI response
      updateIssueAssignee(issueId, newAssignee);
      
      // Then send the update to the server
      const formData = new FormData();
      formData.append('_action', 'update');
      formData.append('id', issueId);
      formData.append('assigneeId', newAssignee?.id || '');
      
      // Always submit to the issues route as a fetch request instead of navigation
      submit(formData, { 
        method: 'post',
        action: '/issues', // Explicitly target the issues route which has the action
        navigate: false // This prevents navigation and keeps the current route
      });
    }
  };

  const renderAvatar = () => {
    if (currentAssignee) {
      return (
        <Avatar className="size-6 shrink-0">
          <AvatarImage src={currentAssignee.image || ''} alt={currentAssignee.name} />
          <AvatarFallback>{currentAssignee.name[0]}</AvatarFallback>
        </Avatar>
      );
    } else {
      return (
        <div className="size-6 flex items-center justify-center">
          <CircleUserRound className="size-5 text-zinc-600" />
        </div>
      );
    }
  };

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <button className="relative w-fit focus:outline-none">
          {renderAvatar()}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-[206px]">
        <DropdownMenuLabel>Assign to...</DropdownMenuLabel>
        <DropdownMenuItem
          onClick={(e) => {
            e.stopPropagation();
            handleAssigneeChange(null);
          }}
        >
          <div className="flex items-center gap-2">
            <UserIcon className="h-5 w-5" />
            <span>No assignee</span>
          </div>
          {!currentAssignee && <CheckIcon className="ml-auto h-4 w-4" />}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {users.map((user: User) => (
          <DropdownMenuItem
            key={user.id}
            onClick={(e) => {
              e.stopPropagation();
              handleAssigneeChange(user);
            }}
          >
            <div className="flex items-center gap-2">
              <Avatar className="h-5 w-5">
                <AvatarImage src={user.image || ''} alt={user.name} />
                <AvatarFallback>{user.name[0]}</AvatarFallback>
              </Avatar>
              <span>{user.name}</span>
            </div>
            {currentAssignee?.id === user.id && <CheckIcon className="ml-auto h-4 w-4" />}
          </DropdownMenuItem>
        ))}
        <DropdownMenuSeparator />
        <DropdownMenuLabel>New user</DropdownMenuLabel>
        <DropdownMenuItem>
          <div className="flex items-center gap-2">
            <Send className="h-4 w-4" />
            <span>Invite and assign...</span>
          </div>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
