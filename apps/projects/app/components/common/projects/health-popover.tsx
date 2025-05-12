import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { CircleCheck, CircleX, AlertCircle, HelpCircle, Bell, User } from 'lucide-react';
import { type Project } from '@/mock-data/projects';
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { useIsMobile } from '@/hooks/use-mobile';
import { cn } from '@/lib/utils';

interface HealthPopoverProps {
  project: Project;
}

export function HealthPopover({ project }: HealthPopoverProps) {
  const getHealthIcon = (healthId: string) => {
    switch (healthId) {
      case 'on-track':
        return <CircleCheck className="size-4 text-green-500" />;
      case 'off-track':
        return <CircleX className="size-4 text-red-500" />;
      case 'at-risk':
        return <AlertCircle className="size-4 text-amber-500" />;
      case 'no-update':
      default:
        return <HelpCircle className="size-4 text-muted-foreground" />;
    }
  };

  const isMobile = useIsMobile();

  // Function to safely get the first initial of a name
  const getInitial = (name: string) => {
    return name && name.length > 0 ? name.charAt(0).toUpperCase() : 'U';
  };

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          className="flex items-center justify-center gap-1 h-7 px-2"
          size="sm"
          variant="ghost"
        >
          {getHealthIcon(project.health.id)}
          <span className="text-xs mt-[1px] ml-0.5 hidden xl:inline">
            {project.health.name}
          </span>
        </Button>
      </PopoverTrigger>
      <PopoverContent
        side={isMobile ? 'bottom' : 'left'}
        className={cn('p-0 w-[480px]', isMobile ? 'w-full' : '')}
      >
        <div className="flex items-center justify-between border-b p-3">
          <div className="flex items-center gap-2">
            {project.icon && (
              typeof project.icon === 'string' 
                ? <span className="size-4 shrink-0 text-muted-foreground">{project.icon}</span>
                : <project.icon className="size-4 shrink-0 text-muted-foreground" />
            )}
            <h4 className="font-medium text-sm">{project.name}</h4>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" className="h-7 px-2 text-xs">
              Subscribe
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-7 px-2 text-xs flex items-center gap-1"
            >
              <Bell className="size-3" />
              New update
            </Button>
          </div>
        </div>
        <div className="p-3 space-y-3">
          <div className="flex items-center justify-start gap-3">
            <div className="flex items-center gap-2">
              {getHealthIcon(project.health.id)}
              <span className="text-sm">{project.health.name}</span>
            </div>
            {project.lead ? (
              <div className="flex items-center gap-2">
                <Avatar className="size-5">
                  <AvatarImage 
                    src={project.lead.image || project.lead.avatarUrl} 
                    alt={project.lead.name} 
                  />
                  <AvatarFallback>{getInitial(project.lead.name)}</AvatarFallback>
                </Avatar>
                <span className="text-xs text-muted-foreground">{project.lead.name}</span>
                <span className="text-xs text-muted-foreground">·</span>
                <span className="text-xs text-muted-foreground">
                  {project.startDate ? new Date(project.startDate).toLocaleDateString() : 'No start date'}
                </span>
              </div>
            ) : (
              <div className="flex items-center gap-2">
                <Avatar className="size-5">
                  <AvatarFallback><User className="size-3" /></AvatarFallback>
                </Avatar>
                <span className="text-xs text-muted-foreground">No lead assigned</span>
                {project.startDate && (
                  <>
                    <span className="text-xs text-muted-foreground">·</span>
                    <span className="text-xs text-muted-foreground">
                      {new Date(project.startDate).toLocaleDateString()}
                    </span>
                  </>
                )}
              </div>
            )}
          </div>

          <div>
            <p className="text-sm text-muted-foreground">{project.health.description}</p>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}