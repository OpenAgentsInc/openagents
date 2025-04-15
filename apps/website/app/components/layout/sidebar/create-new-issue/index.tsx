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
import { useIssuesStore, type Issue } from '@/store/issues-store';
import { useCreateIssueStore } from '@/store/create-issue-store';
import { toast } from 'sonner';
import { v4 as uuidv4 } from 'uuid';
import { StatusSelector } from './status-selector';
import { PrioritySelector } from './priority-selector';
import { AssigneeSelector } from './assignee-selector';
import { ProjectSelector } from './project-selector';
import { LabelSelector } from './label-selector';
import { DialogTitle } from '@radix-ui/react-dialog';

export function CreateNewIssue() {
  const [createMore, setCreateMore] = useState<boolean>(false);
  const { isOpen, defaultStatus, openModal, closeModal } = useCreateIssueStore();
  const { addIssue, issues } = useIssuesStore();

  // Generate new ranks since we removed the import
  const generateRank = useCallback(() => {
    // Simple implementation that generates a random rank string
    const chars = 'abcdefghijklmnopqrstuvwxyz';
    return 'a' + 
      chars.charAt(Math.floor(Math.random() * chars.length)) + 
      Math.floor(Math.random() * 10000).toString();
  }, []);

  const generateUniqueIdentifier = useCallback(() => {
    const identifiers = issues.map((issue) => issue.identifier);
    let identifier = Math.floor(Math.random() * 999)
      .toString()
      .padStart(3, '0');
    while (identifiers.includes(`LNUI-${identifier}`)) {
      identifier = Math.floor(Math.random() * 999)
        .toString()
        .padStart(3, '0');
    }
    return identifier;
  }, [issues]);

  const createDefaultData = useCallback(() => {
    const identifier = generateUniqueIdentifier();
    return {
      id: uuidv4(),
      identifier: `LNUI-${identifier}`,
      title: '',
      description: '',
      status: defaultStatus || status.find((s) => s.id === 'to-do')!,
      assignees: null,
      priority: priorities.find((p) => p.id === 'no-priority')!,
      labels: [],
      createdAt: new Date().toISOString(),
      cycleId: '',
      project: undefined,
      subissues: [],
      rank: generateRank(),
    };
  }, [defaultStatus, generateUniqueIdentifier, generateRank]);

  const [addIssueForm, setAddIssueForm] = useState<Issue>(createDefaultData());

  useEffect(() => {
    setAddIssueForm(createDefaultData());
  }, [createDefaultData]);

  const createIssue = () => {
    if (!addIssueForm.title) {
      toast.error('Title is required');
      return;
    }
    toast.success('Issue created');
    addIssue(addIssueForm);
    if (!createMore) {
      closeModal();
    }
    setAddIssueForm(createDefaultData());
  };

  return (
    <Dialog open={isOpen} onOpenChange={(value) => (value ? openModal() : closeModal())}>
      <DialogTrigger asChild>
        <Button className="size-8 shrink-0" variant="secondary" size="icon">
          <RiEditLine />
        </Button>
      </DialogTrigger>
      <DialogContent className="w-full sm:max-w-[750px] p-0 shadow-xl top-[30%]">
        <DialogHeader>
          <DialogTitle>
            <div className="flex items-center px-4 pt-4 gap-2">
              <Button size="sm" variant="outline" className="gap-1.5">
                <Heart className="size-4 text-orange-500 fill-orange-500" />
                <span className="font-medium">CORE</span>
              </Button>
            </div>
          </DialogTitle>
        </DialogHeader>

        <div className="px-4 pb-0 space-y-3 w-full">
          <Input
            className="border-none w-full shadow-none outline-none text-2xl font-medium px-0 h-auto focus-visible:ring-0 overflow-hidden text-ellipsis whitespace-normal break-words"
            placeholder="Issue title"
            value={addIssueForm.title}
            onChange={(e) => setAddIssueForm({ ...addIssueForm, title: e.target.value })}
          />

          <Textarea
            className="border-none w-full shadow-none outline-none resize-none px-0 min-h-16 focus-visible:ring-0 break-words whitespace-normal overflow-wrap"
            placeholder="Add description..."
            value={addIssueForm.description}
            onChange={(e) =>
              setAddIssueForm({ ...addIssueForm, description: e.target.value })
            }
          />

          <div className="w-full flex items-center justify-start gap-1.5 flex-wrap">
            <StatusSelector
              status={addIssueForm.status}
              onChange={(newStatus) =>
                setAddIssueForm({ ...addIssueForm, status: newStatus })
              }
            />
            <PrioritySelector
              priority={addIssueForm.priority}
              onChange={(newPriority) =>
                setAddIssueForm({ ...addIssueForm, priority: newPriority })
              }
            />
            <AssigneeSelector
              assignee={addIssueForm.assignees}
              onChange={(newAssignee) =>
                setAddIssueForm({ ...addIssueForm, assignees: newAssignee })
              }
            />
            <ProjectSelector
              project={addIssueForm.project}
              onChange={(newProject) =>
                setAddIssueForm({ ...addIssueForm, project: newProject })
              }
            />
            <LabelSelector
              selectedLabels={addIssueForm.labels}
              onChange={(newLabels) =>
                setAddIssueForm({ ...addIssueForm, labels: newLabels })
              }
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
            onClick={() => {
              createIssue();
            }}
          >
            Create issue
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
