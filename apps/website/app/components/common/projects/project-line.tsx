import { Project } from '@/mock-data/projects';
import { HealthPopover } from './health-popover';
import { PrioritySelector } from './priority-selector';
import { LeadSelector } from './lead-selector';
import { StatusWithPercent } from './status-with-percent';
import { DatePicker } from './date-picker';

interface ProjectLineProps {
   project: Project;
}

export default function ProjectLine({ project }: ProjectLineProps) {
   return (
      <div className="w-full flex items-center py-3 px-6 border-b hover:bg-sidebar/50 border-muted-foreground/5 text-sm">
         <div className="w-[60%] sm:w-[70%] xl:w-[46%] flex items-center gap-2">
            <div className="relative">
               <div className="inline-flex size-6 bg-muted/50 items-center justify-center rounded shrink-0">
                  <project.icon className="size-4" />
               </div>
            </div>
            <div className="flex flex-col items-start overflow-hidden">
               <span className="font-medium truncate w-full">{project.name}</span>
            </div>
         </div>

         <div className="w-[20%] sm:w-[10%] xl:w-[13%]">
            <HealthPopover project={project} />
         </div>

         <div className="hidden w-[10%] sm:block">
            <PrioritySelector priority={project.priority} />
         </div>
         <div className="hidden xl:block xl:w-[13%]">
            <LeadSelector lead={project.lead} />
         </div>

         <div className="hidden xl:block xl:w-[13%]">
            <DatePicker date={project.startDate ? new Date(project.startDate) : undefined} />
         </div>

         <div className="w-[20%] sm:w-[10%]">
            <StatusWithPercent status={project.status} percentComplete={project.percentComplete} />
         </div>
      </div>
   );
}
