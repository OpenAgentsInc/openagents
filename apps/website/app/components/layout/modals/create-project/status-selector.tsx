import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useEffect, useState } from 'react';
import { getDb } from '@/lib/db/project-helpers';

interface ProjectStatus {
  id: string;
  name: string;
  color: string;
  type: string;
}

interface StatusSelectorProps {
  statusId: string;
  onChange: (statusId: string) => void;
}

export function StatusSelector({ statusId, onChange }: StatusSelectorProps) {
  const [statuses, setStatuses] = useState<ProjectStatus[]>([]);
  const [selectedStatus, setSelectedStatus] = useState<ProjectStatus | null>(null);
  const [loading, setLoading] = useState(true);

  // Fetch statuses on mount
  useEffect(() => {
    async function fetchStatuses() {
      try {
        const db = getDb();
        const results = await db
          .selectFrom('project_status')
          .select(['id', 'name', 'color', 'type'])
          .where('archivedAt', 'is', null)
          .orderBy('position')
          .execute();
        
        setStatuses(results);
        
        // Set default status if none selected and statuses available
        if (!statusId && results.length > 0) {
          const defaultStatus = results.find(s => s.type === 'backlog') || results[0];
          onChange(defaultStatus.id);
          setSelectedStatus(defaultStatus);
        } else if (statusId && results.length > 0) {
          const found = results.find(s => s.id === statusId);
          if (found) setSelectedStatus(found);
        }
        
        setLoading(false);
      } catch (error) {
        console.error('Error fetching project statuses:', error);
        setLoading(false);
      }
    }
    
    fetchStatuses();
  }, [statusId, onChange]);

  const handleStatusChange = (newStatusId: string) => {
    const status = statuses.find(s => s.id === newStatusId);
    if (status) {
      setSelectedStatus(status);
      onChange(newStatusId);
    }
  };

  if (loading) {
    return (
      <Button size="sm" variant="outline" className="gap-1.5" disabled>
        Loading...
      </Button>
    );
  }

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