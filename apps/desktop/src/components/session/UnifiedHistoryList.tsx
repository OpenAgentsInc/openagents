import React from 'react';
import { Button } from '@/components/ui/button';
import { Calendar, FileText, GitBranch, Folder, MessageSquare, RefreshCw, type LucideProps } from 'lucide-react';

// Properly typed Lucide icon components
const CalendarIcon: React.FC<LucideProps> = Calendar;
const FileTextIcon: React.FC<LucideProps> = FileText;
const GitBranchIcon: React.FC<LucideProps> = GitBranch;
const FolderIcon: React.FC<LucideProps> = Folder;
const MessageSquareIcon: React.FC<LucideProps> = MessageSquare;
const RefreshIcon: React.FC<LucideProps> = RefreshCw;
import { useUnifiedHistory, UnifiedSession } from '@/hooks/useUnifiedHistory';

interface UnifiedHistoryListProps {
  limit?: number;
  onSessionSelect?: (session: UnifiedSession) => void;
}

export const UnifiedHistoryList: React.FC<UnifiedHistoryListProps> = ({
  limit = 50,
  onSessionSelect,
}) => {
  const { sessions, isLoading, error, refreshHistory } = useUnifiedHistory(limit);

  const handleSessionClick = (session: UnifiedSession) => {
    if (onSessionSelect) {
      onSessionSelect(session);
    } else {
      // Default action: open file if it's a local session
      if (session.source === 'local' && session.file_path) {
        console.log('Opening local session file:', session.file_path);
        // TODO: Implement file opening logic
      } else if (session.source === 'convex') {
        console.log('Opening Convex session:', session.id);
        // TODO: Implement Convex session opening logic
      }
    }
  };

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / (1000 * 60));
      const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
      const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

      if (diffMins < 1) return 'just now';
      if (diffMins < 60) return `${diffMins}m ago`;
      if (diffHours < 24) return `${diffHours}h ago`;
      if (diffDays < 7) return `${diffDays}d ago`;
      
      return date.toLocaleDateString();
    } catch {
      return 'Unknown time';
    }
  };

  const getSourceIcon = (source: 'local' | 'convex') => {
    return source === 'local' ? (
      <FileTextIcon className="w-3 h-3" />
    ) : (
      <GitBranchIcon className="w-3 h-3" />
    );
  };

  const getSourceColor = (source: 'local' | 'convex') => {
    return source === 'local' ? 'bg-blue-100 text-blue-800' : 'bg-green-100 text-green-800';
  };

  if (error) {
    return (
      <div className="p-4 text-center">
        <p className="text-sm text-red-600 mb-2">Failed to load session history</p>
        <p className="text-xs text-muted-foreground mb-3">{error}</p>
        <Button onClick={refreshHistory} size="sm" variant="outline">
          <RefreshIcon className="w-3 h-3 mr-1" />
          Retry
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">
          Session History
        </h3>
        <Button
          onClick={refreshHistory}
          disabled={isLoading}
          size="sm"
          variant="ghost"
          className="h-6 px-2"
        >
          <RefreshIcon className={`w-3 h-3 ${isLoading ? 'animate-spin' : ''}`} />
        </Button>
      </div>

      <div className="flex-1 overflow-y-auto space-y-2">
        {isLoading && sessions.length === 0 ? (
          <div className="p-4 text-center">
            <div className="animate-spin w-4 h-4 border-2 border-primary border-t-transparent rounded-full mx-auto mb-2" />
            <p className="text-xs text-muted-foreground">Loading session history...</p>
          </div>
        ) : sessions.length === 0 ? (
          <div className="p-4 text-center">
            <p className="text-xs text-muted-foreground">No session history found</p>
          </div>
        ) : (
          sessions.map((session) => (
            <div
              key={`${session.source}-${session.id}`}
              className="p-3 border border-border/20 bg-muted/10 hover:bg-muted/20 cursor-pointer transition-colors rounded-sm"
              onClick={() => handleSessionClick(session)}
            >
              {/* Header with source badge and timestamp */}
              <div className="flex items-center justify-between mb-2">
                <div className={`${getSourceColor(session.source)} text-xs px-1.5 py-0.5 flex items-center gap-1 rounded-full`}>
                  {getSourceIcon(session.source)}
                  {session.source === 'local' ? 'Local' : 'Cloud'}
                </div>
                <span className="text-xs text-muted-foreground flex items-center gap-1">
                  <CalendarIcon className="w-3 h-3" />
                  {formatTimestamp(session.timestamp)}
                </span>
              </div>

              {/* Title */}
              <h4 className="text-sm font-medium mb-1 line-clamp-2">
                {session.title}
              </h4>

              {/* Project path */}
              {session.project_path && (
                <div className="flex items-center gap-1 mb-1">
                  <FolderIcon className="w-3 h-3 text-muted-foreground" />
                  <span className="text-xs text-muted-foreground font-mono truncate">
                    {session.project_path}
                  </span>
                </div>
              )}

              {/* First message preview (for local sessions) */}
              {session.first_message && (
                <p className="text-xs text-muted-foreground line-clamp-2 mb-1">
                  {session.first_message}
                </p>
              )}

              {/* Footer with additional info */}
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <div className="flex items-center gap-3">
                  {session.message_count && (
                    <span className="flex items-center gap-1">
                      <MessageSquareIcon className="w-3 h-3" />
                      {session.message_count} msg{session.message_count !== 1 ? 's' : ''}
                    </span>
                  )}
                  {session.status && (
                    <span className="px-1.5 py-0.5 bg-muted rounded text-xs">
                      {session.status}
                    </span>
                  )}
                </div>
                
                {session.created_by && (
                  <span className="text-xs">
                    by {session.created_by}
                  </span>
                )}
              </div>

              {/* Summary (if available) */}
              {session.summary && (
                <div className="mt-2 p-2 bg-muted/20 rounded text-xs">
                  <span className="text-muted-foreground">Summary: </span>
                  <span>{session.summary}</span>
                </div>
              )}
            </div>
          ))
        )}
      </div>

      {/* Statistics footer */}
      {sessions.length > 0 && (
        <div className="mt-3 pt-2 border-t border-border/20">
          <div className="flex justify-between text-xs text-muted-foreground">
            <span>
              {sessions.filter(s => s.source === 'local').length} local, {' '}
              {sessions.filter(s => s.source === 'convex').length} cloud
            </span>
            <span>{sessions.length} total</span>
          </div>
        </div>
      )}
    </div>
  );
};