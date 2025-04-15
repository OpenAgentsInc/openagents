import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogHeader, DialogTrigger } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Heart, RefreshCw } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { RiEditLine } from '@remixicon/react';
import { useState, useEffect, useCallback } from 'react';
import { priorities } from '@/mock-data/priorities';
import { useCreateIssueStore } from '@/store/create-issue-store';
import { toast } from 'sonner';
import { StatusSelector } from './status-selector';
import { PrioritySelector } from './priority-selector';
import { AssigneeSelector } from './assignee-selector';
import { ProjectSelector } from './project-selector';
import { LabelSelector } from './label-selector';
import { DialogTitle } from '@radix-ui/react-dialog';
import { useSubmit, useRevalidator } from 'react-router'; 
import { useSession } from '@/lib/auth-client';
import { TeamSelector } from './team-selector';

// Helper functions for formatting data (copied from server code for consistency)
function getPriorityName(priority: number): string {
  switch (priority) {
    case 0: return 'No priority';
    case 1: return 'Urgent';
    case 2: return 'High';
    case 3: return 'Medium';
    case 4: return 'Low';
    default: return 'No priority';
  }
}

function getPriorityColor(priority: number): string {
  switch (priority) {
    case 0: return '#6B7280'; // Gray
    case 1: return '#EF4444'; // Red
    case 2: return '#F59E0B'; // Amber
    case 3: return '#3B82F6'; // Blue
    case 4: return '#10B981'; // Green
    default: return '#6B7280'; // Gray
  }
}

// Define the issue data interface for form submission
interface IssueFormData {
  title: string;
  description: string;
  teamId: string;
  stateId: string;
  priority: number;
  assigneeId?: string;
  projectId?: string;
  labelIds: string[];
}

interface CreateNewIssueProps {
  loaderData?: any;
  initialProjectId?: string;
}

export function CreateNewIssue({ loaderData, initialProjectId }: CreateNewIssueProps) {
  const [createMore, setCreateMore] = useState<boolean>(false);
  const [isSubmitting, setIsSubmitting] = useState<boolean>(false);
  const { isOpen, defaultStatus, openModal, closeModal } = useCreateIssueStore();
  const submit = useSubmit();
  const revalidator = useRevalidator();
  const { data: session, isLoading } = useSession();
  
  // Generate new ranks for UI only
  const generateRank = useCallback(() => {
    // Simple implementation that generates a random rank string
    const chars = 'abcdefghijklmnopqrstuvwxyz';
    return 'a' + 
      chars.charAt(Math.floor(Math.random() * chars.length)) + 
      Math.floor(Math.random() * 10000).toString();
  }, []);

  const generateUniqueIdentifier = useCallback(() => {
    // Simple random identifier for UI only
    return Math.floor(Math.random() * 999).toString().padStart(3, '0');
  }, []);

  const createDefaultFormData = useCallback((): IssueFormData => {
    return {
      title: '',
      description: '',
      teamId: '',
      stateId: '',
      priority: 0, // No priority
      assigneeId: undefined,
      projectId: initialProjectId, // Use the project ID from the URL if available
      labelIds: [],
    };
  }, [initialProjectId]);

  const [issueForm, setIssueForm] = useState<IssueFormData>(createDefaultFormData());

  useEffect(() => {
    if (isOpen) {
      setIssueForm(createDefaultFormData());
    }
  }, [isOpen, createDefaultFormData]);

  // Set default status when opening modal
  useEffect(() => {
    if (defaultStatus) {
      setIssueForm(prev => ({
        ...prev,
        stateId: defaultStatus.id
      }));
    }
  }, [defaultStatus, isOpen]);
  
  // When team changes, reset the state selection
  useEffect(() => {
    if (issueForm.teamId) {
      // Get workflow states for this team
      let teamWorkflowStates = [];
      if (loaderData?.options?.workflowStates) {
        teamWorkflowStates = loaderData.options.workflowStates.filter(
          (state: any) => state.teamId === issueForm.teamId || state.teamId === null
        );
      }
      
      // Set a default state
      if (teamWorkflowStates.length > 0) {
        // Find a backlog or todo state, or use the first state
        const defaultState = teamWorkflowStates.find(
          (state: any) => state.type === 'todo' || state.type === 'backlog' || state.type === 'unstarted'
        ) || teamWorkflowStates[0];
        
        setIssueForm(prev => ({
          ...prev,
          stateId: defaultState.id
        }));
      }
    }
  }, [issueForm.teamId, loaderData]);
  
  // Store a reference to the form data for other components to access
  useEffect(() => {
    (window as any).__createIssueFormContext = issueForm;
  }, [issueForm]);

  const createIssue = async () => {
    if (!session?.user) {
      toast.error('You must be logged in to create an issue');
      return;
    }

    if (!issueForm.title) {
      toast.error('Title is required');
      return;
    }

    if (!issueForm.teamId) {
      toast.error('Team is required');
      return;
    }

    if (!issueForm.stateId) {
      toast.error('Status is required');
      return;
    }

    // Set submitting state to show spinner
    setIsSubmitting(true);

    try {
      // Submit the form to create a new issue using our route action
      const formData = new FormData();
      formData.append('_action', 'create');
      formData.append('title', issueForm.title);
      formData.append('description', issueForm.description || '');
      formData.append('teamId', issueForm.teamId);
      formData.append('stateId', issueForm.stateId);
      formData.append('priority', issueForm.priority.toString());
      
      if (issueForm.assigneeId) {
        formData.append('assigneeId', issueForm.assigneeId);
      }
      
      if (issueForm.projectId) {
        formData.append('projectId', issueForm.projectId);
      }
      
      issueForm.labelIds.forEach(labelId => {
        formData.append('labelIds', labelId);
      });
  
      // Submit the form
      await submit(formData, {
        method: 'post',
        navigate: false,
        action: '/issues'
      });
      
      // After successful submission, revalidate the data
      revalidator.revalidate();

      // We'll simply let the revalidator do its job to refresh the data
      // This is the most reliable approach and avoids state management issues
  
      toast.success('Issue created! Refreshing data...');
  
      if (!createMore) {
        closeModal();
      }
  
      setIssueForm(createDefaultFormData());
    } catch (error) {
      console.error('Error creating issue:', error);
      toast.error('Failed to create issue. Please try again.');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(value) => (value ? openModal() : closeModal())}>
      <DialogTrigger asChild>
        <Button className="size-8 shrink-0" variant="secondary" size="icon">
          <RiEditLine />
        </Button>
      </DialogTrigger>
      <DialogContent className="w-full sm:max-w-[750px] p-0 shadow-xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            <div className="flex items-center px-4 pt-4 gap-2">
              <h2 className="text-lg font-medium">Create New Issue</h2>
            </div>
          </DialogTitle>
        </DialogHeader>

        <div className="px-4 pb-0 space-y-3 w-full">
          <Input
            className="border-none w-full shadow-none outline-none text-2xl font-medium px-0 h-auto focus-visible:ring-0 overflow-hidden text-ellipsis whitespace-normal break-words"
            placeholder="Issue title"
            value={issueForm.title}
            onChange={(e) => setIssueForm({ ...issueForm, title: e.target.value })}
          />

          <Textarea
            className="border-none w-full shadow-none outline-none resize-none px-0 min-h-16 focus-visible:ring-0 break-words whitespace-normal overflow-wrap"
            placeholder="Add description..."
            value={issueForm.description}
            onChange={(e) => setIssueForm({ ...issueForm, description: e.target.value })}
          />

          <div className="w-full flex flex-wrap gap-1.5 py-2">
            <TeamSelector
              teamId={issueForm.teamId}
              onChange={(newTeamId) => setIssueForm({ ...issueForm, teamId: newTeamId })}
              loaderData={loaderData}
            />
            
            <StatusSelector
              stateId={issueForm.stateId}
              onChange={(newStateId) => setIssueForm({ ...issueForm, stateId: newStateId })}
              loaderData={loaderData}
            />
                        
            <PrioritySelector
              priority={issueForm.priority}
              onChange={(newPriority) => setIssueForm({ ...issueForm, priority: newPriority })}
            />
            
            <AssigneeSelector
              assigneeId={issueForm.assigneeId}
              onChange={(newAssigneeId) => setIssueForm({ ...issueForm, assigneeId: newAssigneeId })}
              loaderData={loaderData}
            />
                        
            <ProjectSelector
              projectId={issueForm.projectId}
              onChange={(newProjectId) => setIssueForm({ ...issueForm, projectId: newProjectId })}
              loaderData={loaderData}
            />
            
            <LabelSelector
              selectedLabelIds={issueForm.labelIds}
              onChange={(newLabelIds) => setIssueForm({ ...issueForm, labelIds: newLabelIds })}
              loaderData={loaderData}
            />
          </div>
        </div>

        <div className="flex items-center justify-between py-2.5 px-4 w-full border-t">
          <div className="flex items-center gap-2">
            <div className="flex items-center space-x-2">
              <Switch
                id="create-more"
                checked={createMore}
                onCheckedChange={setCreateMore}
              />
              <Label htmlFor="create-more">Create more</Label>
            </div>
          </div>
          <Button 
            size="sm"
            onClick={createIssue}
            disabled={isLoading || !session?.user || isSubmitting}
          >
            {isSubmitting ? (
              <>
                <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                Creating...
              </>
            ) : (
              <>
                <Heart className="mr-2 h-4 w-4" />
                Create issue
              </>
            )}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}