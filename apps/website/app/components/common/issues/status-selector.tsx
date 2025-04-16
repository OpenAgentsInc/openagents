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
import { useIssuesStore } from '@/store/issues-store';
import { CheckIcon, CheckCircle, Circle, Clock, Hourglass, Timer } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useLoaderData, useSubmit } from 'react-router';

// Define a more generic Status interface compatible with DB data
interface Status {
  id: string;
  name: string;
  color: string;
  type?: string;
}

// Component to render appropriate icon based on status type
const StatusIcon = ({ status }: { status: Status }) => {
  const type = status.type?.toLowerCase() || '';

  if (type.includes('done') || type.includes('completed')) {
    return <CheckCircle className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('progress') || type.includes('started')) {
    return <Hourglass className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('todo')) {
    return <Circle className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('backlog')) {
    return <Clock className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('triage')) {
    return <Timer className="size-4" style={{ color: status.color }} />;
  }

  // Default icon if no matching type
  return <Circle className="size-4" style={{ color: status.color }} />;
};

interface StatusSelectorProps {
  status: Status;
  issueId: string;
}

export function StatusSelector({ status, issueId }: StatusSelectorProps) {
  const id = useId();
  const [open, setOpen] = useState<boolean>(false);
  const [value, setValue] = useState<string>(status.id);
  const loaderData = useLoaderData() || {};
  const submit = useSubmit();

  // Get workflow states from loader data
  let workflowStates: Status[] = [];

  if (loaderData.options && Array.isArray(loaderData.options.workflowStates)) {
    workflowStates = loaderData.options.workflowStates;
  } else if (Array.isArray(loaderData.workflowStates)) {
    workflowStates = loaderData.workflowStates;
  } else {
    // Try to get all workflow states from the store
    const { getWorkflowStates } = useIssuesStore();
    workflowStates = getWorkflowStates ? getWorkflowStates() : [];
  }

  // Default workflow states as fallback if none are available
  if (!workflowStates || workflowStates.length === 0) {
    workflowStates = [
      { id: 'default-triage', name: 'Triage', color: '#6B7280', type: 'triage' },
      { id: 'default-backlog', name: 'Backlog', color: '#95A5A6', type: 'backlog' },
      { id: 'default-todo', name: 'To Do', color: '#3498DB', type: 'todo' },
      { id: 'default-inprogress', name: 'In Progress', color: '#F1C40F', type: 'inprogress' },
      { id: 'default-done', name: 'Done', color: '#2ECC71', type: 'done' }
    ];
  }

  const { updateIssueStatus, filterByStatus } = useIssuesStore();

  useEffect(() => {
    setValue(status.id);
  }, [status.id]);

  const handleStatusChange = (statusId: string) => {
    setValue(statusId);
    setOpen(false);

    if (issueId) {
      const newStatus = workflowStates.find((s) => s.id === statusId);
      if (newStatus) {
        // Update UI state immediately
        updateIssueStatus(issueId, newStatus);
        
        // Then send the update to the server
        const formData = new FormData();
        formData.append('_action', 'update');
        formData.append('id', issueId);
        formData.append('stateId', statusId);
        
        // Always submit to the issues route as a fetch request instead of navigation
        submit(formData, {
          method: 'post',
          action: '/issues', // Explicitly target the issues route which has the action
          navigate: false // This prevents navigation and keeps the current route
        });
      }
    }
  };

  return (
    <div className="*:not-first:mt-2">
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            id={id}
            className="size-7 flex items-center justify-center"
            size="icon"
            variant="ghost"
            role="combobox"
            aria-expanded={open}
          >
            <StatusIcon status={status} />
          </Button>
        </PopoverTrigger>
        <PopoverContent
          className="border-input w-full min-w-[var(--radix-popper-anchor-width)] p-0"
          align="start"
        >
          <Command>
            <CommandInput placeholder="Set status..." />
            <CommandList>
              <CommandEmpty>No status found.</CommandEmpty>
              <CommandGroup>
                {workflowStates.map((item) => (
                  <CommandItem
                    key={item.id}
                    value={item.id}
                    onSelect={() => handleStatusChange(item.id)}
                    className="flex items-center justify-between"
                  >
                    <div className="flex items-center gap-2">
                      <StatusIcon status={item} />
                      {item.name}
                    </div>
                    {value === item.id && <CheckIcon size={16} className="ml-auto" />}
                    <span className="text-muted-foreground text-xs">
                      {filterByStatus ? filterByStatus(item.id).length : 0}
                    </span>
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </div>
  );
}
