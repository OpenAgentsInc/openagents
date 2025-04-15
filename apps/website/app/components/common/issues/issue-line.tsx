'use client';

import { Issue } from '@/mock-data/issues';
import { format } from 'date-fns';
import { AssigneeUser } from './assignee-user';
import { LabelBadge } from './label-badge';
import { PrioritySelector } from './priority-selector';
import { ProjectBadge } from './project-badge';
import { StatusSelector } from './status-selector';
import { motion } from 'motion/react';

export function IssueLine({ issue, layoutId = false }: { issue: Issue; layoutId?: boolean }) {
   return (
      <motion.div
         //href={`/lndev-ui/issue/${issue.identifier}`}
         {...(layoutId && { layoutId: `issue-line-${issue.identifier}` })}
         className="w-full flex items-center justify-start h-11 px-6 hover:bg-sidebar/50"
      >
         <div className="flex items-center gap-0.5">
            <PrioritySelector priority={issue.priority} issueId={issue.id} />
            <span className="text-sm hidden sm:inline-block text-muted-foreground font-medium w-[66px] truncate shrink-0 mr-0.5">
               {issue.identifier}
            </span>
            <StatusSelector status={issue.status} issueId={issue.id} />
         </div>
         <span className="min-w-0 flex items-center justify-start mr-1 ml-0.5">
            <span className="text-xs sm:text-sm font-medium sm:font-semibold truncate">
               {issue.title}
            </span>
         </span>
         <div className="flex items-center justify-end gap-2 ml-auto sm:w-fit">
            <div className="w-3 shrink-0"></div>
            <div className="-space-x-5 hover:space-x-1 lg:space-x-1 items-center justify-end hidden sm:flex duration-200 transition-all">
               <LabelBadge label={issue.labels} />
               {issue.project && <ProjectBadge project={issue.project} />}
            </div>
            <span className="text-xs text-muted-foreground shrink-0 hidden sm:inline-block">
               {format(new Date(issue.createdAt), 'MMM dd')}
            </span>
            <AssigneeUser user={issue.assignees} />
         </div>
      </motion.div>
   );
}
