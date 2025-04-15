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
import { Box, CheckIcon, FolderIcon } from 'lucide-react';
import { useId, useState } from 'react';
import { useLoaderData } from 'react-router';

interface Project {
  id: string;
  name: string;
  icon?: string;
  color?: string;
}

interface ProjectSelectorProps {
  projectId: string | undefined;
  onChange: (projectId: string | undefined) => void;
  loaderData?: any;
}

export function ProjectSelector({ projectId, onChange, loaderData: propLoaderData }: ProjectSelectorProps) {
  const id = useId();
  const [open, setOpen] = useState<boolean>(false);
  const routeLoaderData = useLoaderData() || {};
  // Use passed loaderData prop or fall back to useLoaderData
  const loaderData = propLoaderData || routeLoaderData;
  
  // Check for projects in various locations in the loader data
  let projects: Project[] = [];
  
  if (loaderData.options && Array.isArray(loaderData.options.projects)) {
    projects = loaderData.options.projects;
  } else if (Array.isArray(loaderData.projects)) {
    projects = loaderData.projects;
  } else if (loaderData.project) {
    // If we're on a project detail page, we should at least have the current project
    projects = [loaderData.project];
  }
  
  console.log('Using loader data from props:', !!propLoaderData);
  console.log('Found projects:', projects?.length || 0);

  const handleProjectChange = (newProjectId: string | undefined) => {
    onChange(newProjectId);
    setOpen(false);
  };

  const selectedProject = projects.find(p => p.id === projectId);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          className="flex items-center gap-1.5"
          size="sm"
          variant="outline"
          role="combobox"
          aria-expanded={open}
        >
          {selectedProject ? (
            <>
              <div 
                className="size-4 rounded"
                style={{ backgroundColor: selectedProject.color || '#6366F1' }}
              >
                {selectedProject.icon && (
                  <span className="text-xs text-white flex items-center justify-center h-full">
                    {selectedProject.icon}
                  </span>
                )}
              </div>
              <span>{selectedProject.name}</span>
            </>
          ) : (
            <>
              <FolderIcon className="size-4" />
              <span>No project</span>
            </>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="w-full min-w-[var(--radix-popper-anchor-width)] p-0"
        align="start"
      >
        <Command>
          <CommandInput placeholder="Search projects..." />
          <CommandList>
            <CommandEmpty>No projects found.</CommandEmpty>
            <CommandGroup>
              <CommandItem
                value="no-project"
                onSelect={() => handleProjectChange(undefined)}
                className="flex items-center justify-between"
              >
                <div className="flex items-center gap-2">
                  <FolderIcon className="size-4" />
                  No Project
                </div>
                {projectId === undefined && <CheckIcon size={16} className="ml-auto" />}
              </CommandItem>
              {projects.map((project) => (
                <CommandItem
                  key={project.id}
                  value={project.name}
                  onSelect={() => handleProjectChange(project.id)}
                  className="flex items-center justify-between"
                >
                  <div className="flex items-center gap-2">
                    <div 
                      className="size-4 rounded"
                      style={{ backgroundColor: project.color || '#6366F1' }}
                    >
                      {project.icon && (
                        <span className="text-xs text-white flex items-center justify-center h-full">
                          {project.icon}
                        </span>
                      )}
                    </div>
                    <span>{project.name}</span>
                  </div>
                  {projectId === project.id && <CheckIcon size={16} className="ml-auto" />}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}