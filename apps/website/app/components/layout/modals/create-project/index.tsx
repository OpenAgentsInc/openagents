import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { useState, useEffect } from 'react';
import { useCreateProjectStore } from '@/store/create-project-store';
import { toast } from 'sonner';
import { v4 as uuidv4 } from 'uuid';
import { LeadSelector } from './lead-selector';
import { StatusSelector } from './status-selector';
import { TeamSelector } from './team-selector';
import { IconPicker } from './icon-picker';
import { ColorPicker } from './color-picker';
import { useSubmit } from 'react-router';
import { useSession } from '@/lib/auth-client';

// Define the project data interface
interface ProjectData {
  id: string;
  name: string;
  description: string;
  icon: string;
  color: string;
  startDate: string | null;
  targetDate: string | null;
  statusId: string;
  leadId: string | null;
  teamIds: string[];
  priority: number;
  health: string;
}

export function CreateProject() {
  const [createMore, setCreateMore] = useState<boolean>(false);
  const { isOpen, openModal, closeModal } = useCreateProjectStore();
  const submit = useSubmit();
  const { data: session, isLoading } = useSession();
  // console.log({ session, isLoading })

  const createDefaultData = (): ProjectData => {
    return {
      id: uuidv4(),
      name: '',
      description: '',
      icon: '📋',
      color: '#6366F1',
      startDate: new Date().toISOString().split('T')[0],
      targetDate: null,
      statusId: '', // Will be set from available statuses
      leadId: null,
      teamIds: [],
      priority: 0,
      health: 'onTrack',
    };
  };

  const [projectForm, setProjectForm] = useState<ProjectData>(createDefaultData());

  useEffect(() => {
    if (isOpen) {
      setProjectForm(createDefaultData());
    }
  }, [isOpen]);

  const createProject = () => {
    if (!session?.user) {
      toast.error('You must be logged in to create a project');
      return;
    }

    if (!projectForm.name) {
      toast.error('Project name is required');
      return;
    }

    if (!projectForm.statusId) {
      toast.error('Project status is required');
      return;
    }

    // Submit the form to create a new project
    const formData = new FormData();
    formData.append('action', 'createProject');
    formData.append('project', JSON.stringify(projectForm));

    submit(formData, {
      method: 'post',
      navigate: false,
    });

    toast.success('Project created');

    if (!createMore) {
      closeModal();
    }

    setProjectForm(createDefaultData());
  };

  return (
    <Dialog open={isOpen} onOpenChange={(value) => (value ? openModal() : closeModal())}>
      <DialogContent className="w-full sm:max-w-[750px] p-0 shadow-xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            <div className="flex items-center px-4 pt-4 gap-2">
              <h2 className="text-lg font-medium">Create New Project</h2>
            </div>
          </DialogTitle>
        </DialogHeader>

        <div className="px-4 pb-0 space-y-3 w-full">
          <Input
            className="border-none w-full shadow-none outline-none text-2xl font-medium px-0 h-auto focus-visible:ring-0 overflow-hidden text-ellipsis whitespace-normal break-words"
            placeholder="Project name"
            value={projectForm.name}
            onChange={(e) => setProjectForm({ ...projectForm, name: e.target.value })}
          />

          <Textarea
            className="border-none w-full shadow-none outline-none resize-none px-0 min-h-16 focus-visible:ring-0 break-words whitespace-normal overflow-wrap"
            placeholder="Add description..."
            value={projectForm.description}
            onChange={(e) => setProjectForm({ ...projectForm, description: e.target.value })}
          />

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 py-2">
            <div className="space-y-2">
              <Label>Icon</Label>
              <IconPicker
                icon={projectForm.icon}
                onChange={(newIcon) => setProjectForm({ ...projectForm, icon: newIcon })}
              />
            </div>

            <div className="space-y-2">
              <Label>Color</Label>
              <ColorPicker
                color={projectForm.color}
                onChange={(newColor) => setProjectForm({ ...projectForm, color: newColor })}
              />
            </div>
          </div>

          <div className="w-full flex items-center justify-start gap-1.5 flex-wrap py-2">
            <StatusSelector
              statusId={projectForm.statusId}
              onChange={(newStatusId) => setProjectForm({ ...projectForm, statusId: newStatusId })}
            />

            <LeadSelector
              leadId={projectForm.leadId}
              onChange={(newLeadId) => setProjectForm({ ...projectForm, leadId: newLeadId })}
            />

            <TeamSelector
              selectedTeamIds={projectForm.teamIds}
              onChange={(newTeamIds) => setProjectForm({ ...projectForm, teamIds: newTeamIds })}
            />
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 py-2">
            <div className="space-y-2">
              <Label>Start Date</Label>
              <Input
                type="date"
                value={projectForm.startDate || ''}
                onChange={(e) => setProjectForm({ ...projectForm, startDate: e.target.value || null })}
              />
            </div>

            <div className="space-y-2">
              <Label>Target Date</Label>
              <Input
                type="date"
                value={projectForm.targetDate || ''}
                onChange={(e) => setProjectForm({ ...projectForm, targetDate: e.target.value || null })}
              />
            </div>
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
            onClick={createProject}
            disabled={isLoading || !session?.user}
          >
            Create project
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
