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

interface WorkflowState {
  id: string;
  name: string;
  color: string;
  type?: string;
}

// Default workflow states as fallback if none are available from the server
const defaultWorkflowStates: WorkflowState[] = [
  { id: 'default-triage', name: 'Triage', color: '#6B7280', type: 'triage' },
  { id: 'default-backlog', name: 'Backlog', color: '#95A5A6', type: 'backlog' },
  { id: 'default-todo', name: 'To Do', color: '#3498DB', type: 'todo' },
  { id: 'default-inprogress', name: 'In Progress', color: '#F1C40F', type: 'inprogress' },
  { id: 'default-done', name: 'Done', color: '#2ECC71', type: 'done' },
  { id: 'default-canceled', name: 'Canceled', color: '#E74C3C', type: 'canceled' }
];

interface StatusSelectorProps {
  stateId: string;
  onChange: (stateId: string) => void;
  loaderData?: any;
}

export function StatusSelector({ stateId, onChange, loaderData: propLoaderData }: StatusSelectorProps) {
  const [open, setOpen] = useState(false);
  const routeLoaderData = useLoaderData() || {};
  // Use passed loaderData prop or fall back to useLoaderData
  const loaderData = propLoaderData || routeLoaderData;
  
  // Check for workflow states in the loader data
  let allWorkflowStates: WorkflowState[] = [];
  
  if (loaderData.options && Array.isArray(loaderData.options.workflowStates)) {
    allWorkflowStates = loaderData.options.workflowStates;
  } else if (Array.isArray(loaderData.workflowStates)) {
    allWorkflowStates = loaderData.workflowStates;
  }
  
  // Get a reference to the CreateNewIssue form context
  const formContext = (window as any).__createIssueFormContext;
  const selectedTeamId = formContext?.teamId || '';
  
  // Filter workflow states by selected team if available
  let workflowStates = allWorkflowStates;
  if (selectedTeamId) {
    console.log('[DEBUG] Filtering workflowStates for teamId:', selectedTeamId);
    workflowStates = allWorkflowStates.filter(state => 
      state.teamId === selectedTeamId || state.teamId === null
    );
  }
  
  console.log('Loader data in status selector:', JSON.stringify(loaderData, null, 2).substring(0, 200) + '...');
  console.log('Using loader data from props:', !!propLoaderData);
  console.log('Found workflow states:', workflowStates?.length || 0);
  
  // Use default states if none are available
  if (!workflowStates || workflowStates.length === 0) {
    console.log('Using default workflow states');
    workflowStates = defaultWorkflowStates;
  }

  // --- START DEBUG LOG ---
  console.log('[DEBUG] StatusSelector Props:', { stateId, workflowStates });
  if (Array.isArray(workflowStates)) {
      console.log(`[DEBUG] StatusSelector - Rendering ${workflowStates.length} states:`, JSON.stringify(workflowStates));
      workflowStates.forEach((state, index) => {
          console.log(`[DEBUG] State ${index}:`, JSON.stringify(state));
      });
  } else {
      console.error('[DEBUG] StatusSelector - workflowStates is NOT an array:', workflowStates);
  }
  // --- END DEBUG LOG ---

  const [selectedState, setSelectedState] = useState<WorkflowState | null>(null);

  // Update selected state when stateId or workflowStates change
  useEffect(() => {
    if (stateId && workflowStates.length > 0) {
      const found = workflowStates.find(s => s.id === stateId);
      if (found) setSelectedState(found);
    } else if (!stateId && workflowStates.length > 0) {
      // Set default state if none selected and states available
      const defaultState = workflowStates.find(state => 
        state.type === 'todo' || state.type === 'backlog' || state.type === 'unstarted'
      ) || workflowStates[0];
      
      onChange(defaultState.id);
      setSelectedState(defaultState);
    }
  }, [stateId, onChange, workflowStates]);

  const handleStateChange = (newStateId: string) => {
    const state = workflowStates.find(s => s.id === newStateId);
    if (state) {
      setSelectedState(state);
      onChange(newStateId);
    }
  };

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <Button size="sm" variant="outline" className="gap-1.5">
          {selectedState ? (
            <>
              <div 
                className="size-3 rounded-full" 
                style={{ backgroundColor: selectedState.color }}
              />
              <span>{selectedState.name}</span>
            </>
          ) : (
            'Select Status'
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-56">
        <DropdownMenuRadioGroup value={stateId} onValueChange={handleStateChange}>
          {workflowStates.map((state) => (
            <DropdownMenuRadioItem key={state.id} value={state.id}>
              <div className="flex items-center gap-2">
                <div 
                  className="size-3 rounded-full" 
                  style={{ backgroundColor: state.color }}
                />
                <span>{state.name}</span>
              </div>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}