import { HealthPopover } from './health-popover';
import { PrioritySelector } from './priority-selector';
import { LeadSelector } from './lead-selector';
import { StatusWithPercent } from './status-with-percent';
import { DatePicker } from './date-picker';
import { Link } from "react-router";
import { IconLoader } from '@/components/ui/icon-loader';

interface Status {
  id: string;
  name: string;
  color: string;
}

interface Priority {
  id: string;
  name: string;
  color: string;
}

interface Health {
  id: string;
  name: string;
  color: string;
  description: string;
}

interface User {
  id: string;
  name: string;
  image: string | null;
}

interface Project {
  id: string;
  name: string;
  icon: string;
  color: string;
  percentComplete: number;
  startDate: string | null;
  targetDate: string | null;
  status: Status;
  priority: Priority;
  health: Health;
  lead: User | null;
}

interface ProjectLineProps {
  project: Project;
}

export default function ProjectLine({ project }: ProjectLineProps) {
  return (
    <Link
      to={`/projects/${project.id}`}
      className="w-full flex items-center py-3 px-6 border-b hover:bg-sidebar/50 border-muted-foreground/5 text-sm group cursor-pointer"
    >
      <div className="w-[60%] sm:w-[70%] xl:w-[46%] flex items-center gap-2">
        <div className="relative">
          <div 
            className="inline-flex size-6 items-center justify-center rounded shrink-0"
            style={{ backgroundColor: project.color || '#6366F1' }}
          >
            <IconLoader icon={project.icon} size={16} />
          </div>
        </div>
        <div className="flex flex-col items-start overflow-hidden">
          <span className="font-medium truncate w-full group-hover:text-primary transition-colors">{project.name}</span>
        </div>
      </div>

      <div className="w-[20%] sm:w-[10%] xl:w-[13%]">
        <HealthPopover project={project} />
      </div>

      <div className="hidden w-[10%] sm:block">
        <PrioritySelector priority={project.priority} />
      </div>
      <div className="hidden xl:block xl:w-[13%]">
        {project.lead && <LeadSelector lead={project.lead} />}
      </div>

      <div className="hidden xl:block xl:w-[13%]">
        <DatePicker date={project.targetDate ? new Date(project.targetDate) : undefined} />
      </div>

      <div className="w-[20%] sm:w-[10%]">
        <StatusWithPercent status={project.status} percentComplete={project.percentComplete} />
      </div>
    </Link>
  );
}