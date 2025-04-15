import { status } from '@/mock-data/status';
import { useIssuesStore } from '@/store/issues-store';
import { useSearchStore } from '@/store/search-store';
import { useViewStore } from '@/store/view-store';
import { type FC } from 'react';
import { DndProvider } from 'react-dnd';
import { HTML5Backend } from 'react-dnd-html5-backend';
import { GroupIssues } from './group-issues';
import { SearchIssues } from './search-issues';
import { CustomDragLayer } from './issue-grid';
import { cn } from '@/lib/utils';
import { useLoaderData } from 'react-router-dom';

export interface AppLoaderData {
  issues: any[];
  workflowStates: any[];
  labels: any[];
  projects: any[];
  teams: any[];
  users: any[];
}

export default function AllIssues() {
  const { isSearchOpen, searchQuery } = useSearchStore();
  const { viewType } = useViewStore();
  const { workflowStates = status } = useLoaderData<AppLoaderData>();

  const isSearching = isSearchOpen && searchQuery.trim() !== '';
  const isViewTypeGrid = viewType === 'grid';

  return (
    <div className={cn('w-full h-full', isViewTypeGrid && 'overflow-x-auto')}>
      {isSearching ? (
        <SearchIssuesView />
      ) : (
        <GroupIssuesListView isViewTypeGrid={isViewTypeGrid} states={workflowStates} />
      )}
    </div>
  );
}

const SearchIssuesView = () => (
  <div className="px-6 mb-6">
    <SearchIssues />
  </div>
);

const GroupIssuesListView: FC<{
  isViewTypeGrid: boolean;
  states: any[];
}> = ({ isViewTypeGrid = false, states = status }) => {
  const { issuesByStatus } = useIssuesStore();
  return (
    <DndProvider backend={HTML5Backend}>
      <CustomDragLayer />
      <div className={cn(isViewTypeGrid && 'flex h-full gap-3 px-2 py-2 min-w-max')}>
        {states.map((statusItem) => (
          <GroupIssues
            key={statusItem.id}
            status={statusItem}
            issues={issuesByStatus[statusItem.id] || []}
            count={issuesByStatus[statusItem.id]?.length || 0}
          />
        ))}
      </div>
    </DndProvider>
  );
};