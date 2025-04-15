import { status } from '@/mock-data/status';
import { useIssuesStore } from '@/store/issues-store';
import { useSearchStore } from '@/store/search-store';
import { useViewStore } from '@/store/view-store';
import { type FC, useEffect } from 'react';
import { DndProvider } from 'react-dnd';
import { HTML5Backend } from 'react-dnd-html5-backend';
import { GroupIssues } from './group-issues';
import { SearchIssues } from './search-issues';
import { CustomDragLayer } from './issue-grid';
import { cn } from '@/lib/utils';
import { useLoaderData } from 'react-router';

export interface AppLoaderData {
  issues: any[];
  options: {
    workflowStates: any[];
    labels: any[];
    projects: any[];
    teams: any[];
    users: any[];
  };
}

export default function AllIssues() {
  const { isSearchOpen, searchQuery } = useSearchStore();
  const { viewType } = useViewStore();
  const { issues, setIssues, setWorkflowStates } = useIssuesStore();
  const loaderData = useLoaderData<AppLoaderData>();
  const workflowStates = loaderData?.options?.workflowStates || status;
  
  // Initialize store with loader data if needed
  useEffect(() => {
    if (loaderData) {
      if (loaderData.issues && loaderData.issues.length > 0) {
        console.log('[DEBUG] AllIssues - Setting issues from loader data:', loaderData.issues.length);
        setIssues(loaderData.issues);
      }
      
      if (loaderData.options?.workflowStates && loaderData.options.workflowStates.length > 0) {
        console.log('[DEBUG] AllIssues - Setting workflow states from loader data:', loaderData.options.workflowStates.length);
        setWorkflowStates(loaderData.options.workflowStates);
      }
    }
  }, [loaderData, setIssues, setWorkflowStates]);
  
  // Debug issues store
  useEffect(() => {
    console.log('[DEBUG] AllIssues - Current issues in store:', issues.length);
  }, [issues]);

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
  const { issuesByStatus, issues } = useIssuesStore();
  
  console.log('[DEBUG] GroupIssuesListView - Issues by status:', Object.keys(issuesByStatus));
  console.log('[DEBUG] GroupIssuesListView - Total issues:', issues.length);
  console.log('[DEBUG] GroupIssuesListView - Available states:', states.map(s => s.id));
  
  // Sort states by position if available, or fall back to order in the array
  const sortedStates = [...states].sort((a, b) => {
    // Sort by position if available
    if (a.position !== undefined && b.position !== undefined) {
      return a.position - b.position;
    }
    
    // Otherwise sort by type using a predefined order
    const typeOrder: Record<string, number> = {
      'triage': 1,
      'backlog': 2, 
      'todo': 3,
      'unstarted': 3,
      'inprogress': 4,
      'started': 4,
      'review': 5, 
      'done': 6,
      'completed': 6,
      'canceled': 7
    };
    
    const aType = (a.type || '').toLowerCase();
    const bType = (b.type || '').toLowerCase();
    
    return (typeOrder[aType] || 99) - (typeOrder[bType] || 99);
  });
  
  return (
    <DndProvider backend={HTML5Backend}>
      <CustomDragLayer />
      <div className={cn(isViewTypeGrid && 'flex h-full gap-3 px-2 py-2 min-w-max')}>
        {sortedStates.map((statusItem) => {
          // Find issues for this status
          const statusIssues = issuesByStatus[statusItem.id] || [];
          console.log(`[DEBUG] Status ${statusItem.name} (${statusItem.id}) has ${statusIssues.length} issues`);
          
          return (
            <GroupIssues
              key={statusItem.id}
              status={statusItem}
              issues={statusIssues}
              count={statusIssues.length}
            />
          );
        })}
      </div>
    </DndProvider>
  );
};
