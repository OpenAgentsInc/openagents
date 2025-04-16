import { type Issue } from '@/store/issues-store';
import { format } from 'date-fns';
import { AssigneeUser } from './assignee-user';
import { LabelBadge } from './label-badge';
import { PrioritySelector } from './priority-selector';
import { ProjectBadge } from './project-badge';
import { StatusSelector } from './status-selector';
import { motion } from 'motion/react';
import { useNavigate } from 'react-router';

export function IssueLine({ issue, layoutId = false }: { issue: Issue; layoutId?: boolean }) {
  const navigate = useNavigate();

  // Handler to navigate to issue detail page
  const handleIssueClick = (e: React.MouseEvent) => {
    // Prevent navigation if the click was on an interactive element
    const target = e.target as HTMLElement;
    if (
      target.closest('.stop-propagation') ||
      target.closest('button') ||
      target.closest('select') ||
      target.closest('[role="button"]')
    ) {
      return;
    }
    navigate(`/issues/${issue.id}`);
  };

  return (
    <motion.div
      {...(layoutId && { layoutId: `issue-line-${issue.identifier}` })}
      className="w-full flex items-center justify-start h-11 px-6 hover:bg-sidebar/50 cursor-pointer"
      onClick={handleIssueClick}
    >
      <div className="flex items-center gap-0.5 stop-propagation">
        <PrioritySelector priority={issue.priority} issueId={issue.id} />
        <span className="text-sm hidden sm:inline-block text-muted-foreground font-medium w-[85px] truncate shrink-0 mr-0.5">
          {issue.identifier}
        </span>
        <StatusSelector status={issue.status} issueId={issue.id} />
      </div>
      <span className="min-w-0 flex items-center justify-start mr-1 ml-0.5">
        <span className="text-xs sm:text-sm font-medium truncate">
          {issue.title}
        </span>
      </span>
      <div className="flex items-center justify-end gap-2 ml-auto sm:w-fit stop-propagation">
        <div className="w-3 shrink-0"></div>
        <div className="-space-x-5 hover:space-x-1 lg:space-x-1 items-center justify-end hidden sm:flex duration-200 transition-all">
          {issue.labels && issue.labels.length > 0 && <LabelBadge label={issue.labels} />}
          {issue.project && <ProjectBadge project={issue.project} />}
        </div>
        {issue.createdAt && (
          <span className="text-xs text-muted-foreground shrink-0 hidden sm:inline-block">
            {format(new Date(issue.createdAt), 'MMM dd')}
          </span>
        )}
        <AssigneeUser user={issue.assignee} issueId={issue.id} />
      </div>
    </motion.div>
  );
}