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
    console.log('Warning: No workflow states found, using defaults in UI');
    workflowStates = [
      { id: 'default-triage', name: 'Triage', color: '#6B7280', type: 'triage' },
      { id: 'default-backlog', name: 'Backlog', color: '#95A5A6', type: 'backlog' },
      { id: 'default-todo', name: 'To Do', color: '#3498DB', type: 'todo' },
      { id: 'default-inprogress', name: 'In Progress', color: '#F1C40F', type: 'inprogress' },
      { id: 'default-done', name: 'Done', color: '#2ECC71', type: 'done' }
    ];
  } else {
    // Make sure we have at least one of each standard type
    const hasTriageState = workflowStates.some(s => s.type === 'triage');
    const hasBacklogState = workflowStates.some(s => s.type === 'backlog');
    const hasTodoState = workflowStates.some(s => s.type === 'todo');
    const hasInProgressState = workflowStates.some(s => s.type === 'inprogress');
    const hasDoneState = workflowStates.some(s => s.type === 'done');
    
    // Add missing states as defaults
    if (!hasTriageState) {
      workflowStates.push({ id: 'default-triage', name: 'Triage', color: '#6B7280', type: 'triage' });
    }
    if (!hasBacklogState) {
      workflowStates.push({ id: 'default-backlog', name: 'Backlog', color: '#95A5A6', type: 'backlog' });
    }
    if (!hasTodoState) {
      workflowStates.push({ id: 'default-todo', name: 'To Do', color: '#3498DB', type: 'todo' });
    }
    if (!hasInProgressState) {
      workflowStates.push({ id: 'default-inprogress', name: 'In Progress', color: '#F1C40F', type: 'inprogress' });
    }
    if (!hasDoneState) {
      workflowStates.push({ id: 'default-done', name: 'Done', color: '#2ECC71', type: 'done' });
    }
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
        // Extra debugging for Done status
        const isDoneStatus = newStatus.type === 'done' || newStatus.name === 'Done';
        if (isDoneStatus) {
          console.log('DEBUG: Setting status to Done:', {
            statusId,
            statusObj: newStatus,
            issueId
          });
        }
        
        // Update UI state immediately
        updateIssueStatus(issueId, newStatus);
        
        // Then send the update to the server
        const formData = new FormData();
        formData.append('_action', 'update');
        formData.append('id', issueId);
        formData.append('stateId', statusId);
        
        // For Done status, add extra data to ensure proper handling
        if (isDoneStatus) {
          // Add completedAt date
          formData.append('completedAt', new Date().toISOString());
          // Extra flag to identify Done status for debugging server-side
          formData.append('isDone', 'true');
          
          // Get teamId from the current issue if available (from loader data)
          const currentIssue = loaderData?.issues?.find((issue: any) => issue.id === issueId);
          if (currentIssue?.project?.id) {
            console.log(`Adding project ID ${currentIssue.project.id} to done status update`);
            formData.append('projectId', currentIssue.project.id);
          }
          
          if (currentIssue?.teamId) {
            console.log(`Adding team ID ${currentIssue.teamId} to done status update`);
            formData.append('teamId', currentIssue.teamId);
          } else if (currentIssue?.team?.id) {
            console.log(`Adding team ID ${currentIssue.team.id} to done status update`);
            formData.append('teamId', currentIssue.team.id);
          }
        }
        
        // Add debug info to help troubleshoot
        console.log(`Updating issue ${issueId} status to ${statusId} (type: ${newStatus.type})`);
        
        // Always submit to the issues route as a fetch request instead of navigation
        submit(formData, {
          method: 'post',
          action: '/issues', // Explicitly target the issues route which has the action
          navigate: false, // This prevents navigation and keeps the current route
          replace: true // This causes the page state to be updated with the server response
        });
        
        // Add an additional handler for "Done" status to re-sync after a short delay
        if (isDoneStatus) {
          setTimeout(() => {
            console.log('DEBUG: Re-submitting fetch for issue list after Done status update');
            // Fetch a fresh list of issues to ensure store is fully in sync
            submit(null, {
              method: 'get',
              action: '/issues',
              navigate: false,
              replace: true
            });
          }, 500); // Short delay to allow the status update to be processed
        }
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
