import { type Issue } from '@/mock-data/issues';
import { useIssuesStore } from '@/store/issues-store';
import { useViewStore } from '@/store/view-store';
import { cn } from '@/lib/utils';
import { CheckCircle, Circle, Clock, Hourglass, Plus, Timer } from 'lucide-react';
import { type FC, useRef } from 'react';
import { useDrop } from 'react-dnd';
import { Button } from '../../ui/button';
import { IssueDragType, IssueGrid } from './issue-grid';
import { IssueLine } from './issue-line';
import { useCreateIssueStore } from '@/store/create-issue-store';
import { sortIssuesByPriority } from '@/mock-data/issues';
import { AnimatePresence, motion } from 'motion/react';

// Define a more generic Status interface compatible with DB data
interface Status {
  id: string;
  name: string;
  color: string;
  type?: string;
}

// Component to render appropriate icon based on status type
const StatusIcon: FC<{ status: Status }> = ({ status }) => {
  const type = status.type?.toLowerCase() || '';
  
  if (type.includes('done') || type.includes('completed')) {
    return <CheckCircle className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('progress') || type.includes('started')) {
    return <Hourglass className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('todo')) {
    return <Circle className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('backlog')) {
    return <Clock className="size-4" style={{ color: status.color }} />;
  } else if (type.includes('triage')) {
    return <Timer className="size-4" style={{ color: status.color }} />;
  }
  
  // Default icon if no matching type
  return <Circle className="size-4" style={{ color: status.color }} />;
};

interface GroupIssuesProps {
  status: Status;
  issues: Issue[];
  count: number;
}

export function GroupIssues({ status, issues, count }: GroupIssuesProps) {
  const { viewType } = useViewStore();
  const isViewTypeGrid = viewType === 'grid';
  const { openModal } = useCreateIssueStore();
  const sortedIssues = sortIssuesByPriority(issues);

  return (
    <div
      className={cn(
        'bg-conainer',
        isViewTypeGrid
          ? 'overflow-hidden rounded-md h-full flex-shrink-0 w-[348px] flex flex-col'
          : ''
      )}
    >
      <div
        className={cn(
          'sticky top-0 z-10 bg-container w-full',
          isViewTypeGrid ? 'rounded-t-md h-[50px]' : 'h-10'
        )}
      >
        <div
          className={cn(
            'w-full h-full flex items-center justify-between',
            isViewTypeGrid ? 'px-3' : 'px-6'
          )}
          style={{
            backgroundColor: isViewTypeGrid ? `${status.color}10` : `${status.color}08`,
          }}
        >
          <div className="flex items-center gap-2">
            <StatusIcon status={status} />
            <span className="text-sm font-medium">{status.name}</span>
            <span className="text-sm text-muted-foreground">{count}</span>
          </div>

          <Button
            className="size-6"
            size="icon"
            variant="ghost"
            onClick={(e) => {
              e.stopPropagation();
              openModal(status);
            }}
          >
            <Plus className="size-4" />
          </Button>
        </div>
      </div>

      {viewType === 'list' ? (
        <div className="space-y-0">
          {sortedIssues.map((issue) => (
            <IssueLine key={issue.id} issue={issue} layoutId={true} />
          ))}
        </div>
      ) : (
        <IssueGridList issues={issues} status={status} />
      )}
    </div>
  );
}

const IssueGridList: FC<{ issues: Issue[]; status: Status }> = ({ issues, status }) => {
  const ref = useRef<HTMLDivElement>(null);
  const { updateIssueStatus } = useIssuesStore();

  // Set up drop functionality to accept only issue items.
  const [{ isOver }, drop] = useDrop(() => ({
    accept: IssueDragType,
    drop(item: Issue, monitor) {
      if (monitor.didDrop() && item.status.id !== status.id) {
        // Enhanced debugging for "Done" status
        const isDoneStatus = status.type === 'done' || status.name === 'Done';
        if (isDoneStatus) {
          console.log('DEBUG: Drag and drop - Setting issue status to Done:', {
            issueId: item.id,
            oldStatus: item.status,
            newStatus: status
          });
        }
        
        updateIssueStatus(item.id, status);
      }
    },
    collect: (monitor) => ({
      isOver: !!monitor.isOver(),
    }),
  }));
  drop(ref);

  const sortedIssues = sortIssuesByPriority(issues);

  return (
    <div
      ref={ref}
      className="flex-1 h-full overflow-y-auto p-2 space-y-2 bg-zinc-50/50 dark:bg-zinc-900/50 relative"
    >
      <AnimatePresence>
        {isOver && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.1 }}
            className="fixed top-0 left-0 right-0 bottom-0 z-10 flex items-center justify-center pointer-events-none bg-background/90"
            style={{
              width: ref.current?.getBoundingClientRect().width || '100%',
              height: ref.current?.getBoundingClientRect().height || '100%',
              transform: `translate(${ref.current?.getBoundingClientRect().left || 0}px, ${ref.current?.getBoundingClientRect().top || 0}px)`,
            }}
          >
            <div className="bg-background border border-border rounded-md p-3 shadow-md max-w-[90%]">
              <p className="text-sm font-medium text-center">Board ordered by priority</p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      {sortedIssues.map((issue) => (
        <IssueGrid key={issue.id} issue={issue} />
      ))}
    </div>
  );
};
