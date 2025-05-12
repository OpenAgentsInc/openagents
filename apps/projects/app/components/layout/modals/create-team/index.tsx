import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { useState, useEffect } from 'react';
import { useCreateTeamStore } from '@/store/create-team-store';
import { toast } from 'sonner';
import { IconPicker } from './icon-picker';
import { ColorPicker } from './color-picker';
import { useSubmit } from 'react-router';
import { useSession } from '@/lib/auth-client';

// Define the team data interface
interface TeamFormData {
  name: string;
  description: string;
  icon: string;
  color: string;
  private: boolean;
  cyclesEnabled: boolean;
}

export function CreateTeam() {
  const [createMore, setCreateMore] = useState<boolean>(false);
  const { isOpen, openModal, closeModal } = useCreateTeamStore();
  const submit = useSubmit();
  const { data: session, isLoading } = useSession();

  const createDefaultData = (): TeamFormData => {
    return {
      name: '',
      description: '',
      icon: '👥',
      color: '#6366F1',
      private: false,
      cyclesEnabled: false,
    };
  };

  const [teamForm, setTeamForm] = useState<TeamFormData>(createDefaultData());

  useEffect(() => {
    if (isOpen) {
      setTeamForm(createDefaultData());
    }
  }, [isOpen]);

  const createTeam = () => {
    if (!session?.user) {
      toast.error('You must be logged in to create a team');
      return;
    }

    if (!teamForm.name) {
      toast.error('Team name is required');
      return;
    }

    // Submit the form to create a new team
    const formData = new FormData();
    formData.append('_action', 'createTeam');
    formData.append('name', teamForm.name);
    formData.append('description', teamForm.description);
    formData.append('icon', teamForm.icon);
    formData.append('color', teamForm.color);
    formData.append('private', teamForm.private ? '1' : '0');
    formData.append('cyclesEnabled', teamForm.cyclesEnabled ? '1' : '0');

    submit(formData, {
      method: 'post',
      navigate: false,
      action: '/teams'
    });

    toast.success('Team created');

    if (!createMore) {
      closeModal();
    }

    setTeamForm(createDefaultData());
  };

  return (
    <Dialog open={isOpen} onOpenChange={(value) => (value ? openModal() : closeModal())}>
      <DialogContent className="w-full sm:max-w-[550px] p-0 shadow-xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            <div className="flex items-center px-4 pt-4 gap-2">
              <h2 className="text-lg font-medium">Create New Team</h2>
            </div>
          </DialogTitle>
        </DialogHeader>

        <div className="px-4 pb-0 space-y-3 w-full">
          <Input
            className="border-none w-full shadow-none outline-none text-2xl font-medium px-0 h-auto focus-visible:ring-0 overflow-hidden text-ellipsis whitespace-normal break-words"
            placeholder="Team name"
            value={teamForm.name}
            onChange={(e) => setTeamForm({ ...teamForm, name: e.target.value })}
          />

          <Textarea
            className="border-none w-full shadow-none outline-none resize-none px-0 min-h-16 focus-visible:ring-0 break-words whitespace-normal overflow-wrap"
            placeholder="Add description..."
            value={teamForm.description}
            onChange={(e) => setTeamForm({ ...teamForm, description: e.target.value })}
          />

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 py-2">
            <div className="space-y-2">
              <Label>Icon</Label>
              <IconPicker
                icon={teamForm.icon}
                onChange={(newIcon) => setTeamForm({ ...teamForm, icon: newIcon })}
              />
            </div>

            <div className="space-y-2">
              <Label>Color</Label>
              <ColorPicker
                color={teamForm.color}
                onChange={(newColor) => setTeamForm({ ...teamForm, color: newColor })}
              />
            </div>
          </div>

          <div className="flex items-center space-x-2 py-2">
            <Switch
              id="team-private"
              checked={teamForm.private}
              onCheckedChange={(value) => setTeamForm({ ...teamForm, private: value })}
            />
            <Label htmlFor="team-private">Private team</Label>
          </div>

          <div className="flex items-center space-x-2 py-2">
            <Switch
              id="team-cycles"
              checked={teamForm.cyclesEnabled}
              onCheckedChange={(value) => setTeamForm({ ...teamForm, cyclesEnabled: value })}
            />
            <Label htmlFor="team-cycles">Enable cycles</Label>
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
            onClick={createTeam}
            disabled={isLoading || !session?.user}
          >
            Create team
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
