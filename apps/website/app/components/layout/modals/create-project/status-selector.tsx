import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useEffect, useState } from 'react';
import { useLoaderData } from 'react-router';

interface ProjectStatus {
  id: string;
  name: string;
  color: string;
  type: string;
}

// Default statuses as fallback if none are available from the server
const defaultStatuses: ProjectStatus[] = [
  { id: 'default-backlog', name: 'Backlog', color: '#95A5A6', type: 'backlog' },
  { id: 'default-planned', name: 'Planned', color: '#3498DB', type: 'planned' },
  { id: 'default-started', name: 'In Progress', color: '#F1C40F', type: 'started' },
  { id: 'default-completed', name: 'Completed', color: '#2ECC71', type: 'completed' },
  { id: 'default-canceled', name: 'Canceled', color: '#E74C3C', type: 'canceled' }
];

interface StatusSelectorProps {
  statusId: string;
  onChange: (statusId: string) => void;
}

interface LoaderData {
  options: {
    statuses: ProjectStatus[];
    users: any[];
    teams: any[];
  };
}

export function StatusSelector({ statusId, onChange }: StatusSelectorProps) {
  const loaderData = useLoaderData() as LoaderData;
  
  // Debug log to see what's being loaded
  console.log('[DEBUG] Project StatusSelector - loaderData:', JSON.stringify(loaderData || {}, null, 2).substring(0, 300) + '...');
  
  // Get status options from loaderData
  let serverStatuses = loaderData?.options?.statuses || [];
  
  // Debug log for statuses
  console.log('[DEBUG] Project StatusSelector - Server statuses:', serverStatuses.length, JSON.stringify(serverStatuses || []));
  
  // ALWAYS use default statuses - server statuses are too limited
  const statuses = defaultStatuses;
    
  const [selectedStatus, setSelectedStatus] = useState<ProjectStatus | null>(null);

  // Update selected status when statusId or statuses change
  useEffect(() => {
    if (statusId && statuses.length > 0) {
      const found = statuses.find(s => s.id === statusId);
      if (found) setSelectedStatus(found);
    } else if (!statusId && statuses.length > 0) {
      // Set default status if none selected and statuses available
      const defaultStatus = statuses.find(s => s.type === 'backlog') || statuses[0];
      onChange(defaultStatus.id);
      setSelectedStatus(defaultStatus);
    }
  }, [statusId, onChange, statuses]);

  const handleStatusChange = (newStatusId: string) => {
    const status = statuses.find(s => s.id === newStatusId);
    if (status) {
      setSelectedStatus(status);
      onChange(newStatusId);
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="sm" variant="outline" className="gap-1.5">
          {selectedStatus ? (
            <>
              <div 
                className="size-3 rounded-full" 
                style={{ backgroundColor: selectedStatus.color }}
              />
              <span>{selectedStatus.name}</span>
            </>
          ) : (
            'Select Status'
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-56">
        <DropdownMenuRadioGroup value={statusId} onValueChange={handleStatusChange}>
          {statuses.map((status) => (
            <DropdownMenuRadioItem key={status.id} value={status.id}>
              <div className="flex items-center gap-2">
                <div 
                  className="size-3 rounded-full" 
                  style={{ backgroundColor: status.color }}
                />
                <span>{status.name}</span>
              </div>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}