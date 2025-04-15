import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogHeader, DialogTrigger } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Heart } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { RiEditLine } from '@remixicon/react';
import { useState, useEffect, useCallback } from 'react';
import { priorities } from '@/mock-data/priorities';
import { status } from '@/mock-data/status';
import { useIssuesStore } from '@/store/issues-store';
import { useCreateIssueStore } from '@/store/create-issue-store';
import { toast } from 'sonner';
import { v4 as uuidv4 } from 'uuid';
import { StatusSelector } from './status-selector';
import { PrioritySelector } from './priority-selector';
import { AssigneeSelector } from './assignee-selector';
import { ProjectSelector } from './project-selector';
import { LabelSelector } from './label-selector';
import { DialogTitle } from '@radix-ui/react-dialog';
import { useSubmit } from 'react-router'; 
import { useSession } from '@/lib/auth-client';
import { TeamSelector } from './team-selector';

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

export function CreateNewIssue() {
  const [createMore, setCreateMore] = useState<boolean>(false);
  const { isOpen, defaultStatus, openModal, closeModal } = useCreateIssueStore();
  const submit = useSubmit();
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
      stateId: defaultStatus?.id || '',
      priority: 0, // No priority
      assigneeId: undefined,
      projectId: undefined,
      labelIds: [],
    };
  }, [defaultStatus]);

  const [issueForm, setIssueForm] = useState<IssueFormData>(createDefaultFormData());

  useEffect(() => {
    if (isOpen) {
      setIssueForm(createDefaultFormData());
    }
  }, [isOpen, createDefaultFormData]);

  const createIssue = () => {
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

    submit(formData, {
      method: 'post',
      navigate: false,
      action: '/issues'
    });

    toast.success('Issue created');

    if (!createMore) {
      closeModal();
    }

    setIssueForm(createDefaultFormData());
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

          <div className="w-full flex items-center justify-start gap-1.5 flex-wrap py-2">
            <TeamSelector
              teamId={issueForm.teamId}
              onChange={(newTeamId) => setIssueForm({ ...issueForm, teamId: newTeamId })}
            />
            
            <StatusSelector
              status={defaultStatus || status.find((s) => s.id === 'to-do')}
              onChange={(newStatus) => setIssueForm({ ...issueForm, stateId: newStatus.id })}
            />
            
            <PrioritySelector
              priority={priorities.find((p) => p.id === 'no-priority')}
              onChange={(newPriority) => {
                // Convert priority to number based on id
                let priorityNum = 0;
                switch (newPriority.id) {
                  case 'urgent': priorityNum = 1; break;
                  case 'high': priorityNum = 2; break;
                  case 'medium': priorityNum = 3; break;
                  case 'low': priorityNum = 4; break;
                  default: priorityNum = 0;
                }
                setIssueForm({ ...issueForm, priority: priorityNum });
              }}
            />
            
            <AssigneeSelector
              assignee={null}
              onChange={(newAssignee) => setIssueForm({ 
                ...issueForm, 
                assigneeId: newAssignee?.id
              })}
            />
            
            <ProjectSelector
              project={undefined}
              onChange={(newProject) => setIssueForm({ 
                ...issueForm, 
                projectId: newProject?.id
              })}
            />
            
            <LabelSelector
              selectedLabels={[]}
              onChange={(newLabels) => setIssueForm({ 
                ...issueForm, 
                labelIds: newLabels.map(label => label.id)
              })}
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
            disabled={isLoading || !session?.user}
          >
            Create issue
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}