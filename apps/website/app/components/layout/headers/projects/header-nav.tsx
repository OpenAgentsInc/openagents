import { Button } from '@/components/ui/button';
import { SidebarTrigger } from '@/components/ui/sidebar';
import { Plus } from 'lucide-react';
import { useLoaderData } from 'react-router';
import { useCreateProjectStore } from '@/store/create-project-store';
import { CreateProject } from '@/components/layout/modals/create-project';

interface LoaderData {
  projects: any[];
}

export default function HeaderNav() {
  const { projects = [] } = useLoaderData() as LoaderData;
  const { openModal } = useCreateProjectStore();

  return (
    <>
      <div className="w-full flex justify-between items-center border-b py-1.5 px-6 h-10">
        <div className="flex items-center gap-2">
          <SidebarTrigger className="" />
          <div className="flex items-center gap-1">
            <span className="text-sm font-medium">Projects</span>
            <span className="text-xs bg-accent rounded-md px-1.5 py-1">{projects.length}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button 
            className="relative" 
            size="xs" 
            variant="secondary"
            onClick={openModal}
          >
            <Plus className="size-4" />
            <span className="hidden sm:inline ml-1">Create project</span>
          </Button>
        </div>
      </div>
      
      {/* Include the create project modal */}
      <CreateProject />
    </>
  );
}