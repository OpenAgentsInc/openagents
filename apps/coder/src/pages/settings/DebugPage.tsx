import React, { useState, useEffect, useCallback } from "react";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  Button,
  Switch,
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Label,
  Separator,
  Badge,
  ScrollArea,
  Input,
  Alert,
  AlertTitle,
  AlertDescription,
} from "@/components/ui";
import { Toaster, toast as sonnerToast } from "sonner";
import { 
  Trash, 
  Download,
  RefreshCw, 
  Bug,
  Database,
  AlertCircle,
  Info,
  FileWarning,
  FileX,
  Copy,
  Terminal,
  ServerCrash,
  RotateCcw
} from "lucide-react";
import { 
  LogEntry, 
  LogLevel, 
  getAllLogs, 
  filterLogs, 
  clearLogs, 
  setConsoleOutput, 
  getConsoleOutputState, 
  installConsoleInterceptor,
  logger,
  getDatabase,
  cleanupDatabase
} from "@openagents/core";
import { DatabaseErrorNotification } from "@/components/ui/database-error-notification";
import { useDatabaseError } from "@/providers/DatabaseErrorProvider";

// Only log that the debug page has been loaded
// We'll let the user enable console interception manually
if (typeof window !== 'undefined') {
  logger.info('Debug page initialized');
}

export default function DebugPage() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState<LogLevel | 'all'>('all');
  const [moduleFilter, setModuleFilter] = useState<string>('all');
  const [searchFilter, setSearchFilter] = useState<string>('');
  const [refreshing, setRefreshing] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [consoleOutput, setConsoleOutputState] = useState(getConsoleOutputState());
  const [uniqueModules, setUniqueModules] = useState<string[]>([]);
  const [isDbOperationInProgress, setIsDbOperationInProgress] = useState(false);
  
  // Access the database error context
  const { databaseError, setDatabaseError, clearDatabaseError, retryDatabaseOperation } = useDatabaseError();

  // Function to refresh logs
  const refreshLogs = useCallback(() => {
    setRefreshing(true);
    
    try {
      // Get all logs
      const allLogs = getAllLogs();
      
      // Apply filters
      let filteredLogs = allLogs;
      
      // Filter by log level if not 'all'
      if (filter !== 'all') {
        filteredLogs = filterLogs({ level: filter as LogLevel });
      }
      
      // Filter by module if specified and not 'all'
      if (moduleFilter && moduleFilter !== 'all') {
        filteredLogs = filteredLogs.filter(log => log.module === moduleFilter);
      }
      
      // Filter by search text if specified
      if (searchFilter) {
        const searchLower = searchFilter.toLowerCase();
        filteredLogs = filteredLogs.filter(log => 
          log.message.toLowerCase().includes(searchLower) || 
          (log.details && JSON.stringify(log.details).toLowerCase().includes(searchLower))
        );
      }
      
      // Update logs state
      setLogs(filteredLogs);
      
      // Update unique modules list
      const modules = [...new Set(allLogs.map(log => log.module))].sort();
      setUniqueModules(modules);
      
    } catch (error) {
      console.error('Error refreshing logs:', error);
      sonnerToast("Error refreshing logs", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    } finally {
      setRefreshing(false);
    }
  }, [filter, moduleFilter, searchFilter]);

  // Set up automatic refresh if enabled
  useEffect(() => {
    if (autoRefresh) {
      const interval = setInterval(refreshLogs, 1000);
      return () => clearInterval(interval);
    }
  }, [autoRefresh, refreshLogs]);

  // Initial load
  useEffect(() => {
    refreshLogs();
  }, [refreshLogs]);

  // Handle clearing logs
  const handleClearLogs = () => {
    try {
      clearLogs();
      setLogs([]);
      sonnerToast("Logs cleared successfully", {
        description: "All log entries have been removed from memory"
      });
    } catch (error) {
      console.error('Error clearing logs:', error);
      sonnerToast("Error clearing logs", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };

  // Handle toggling console output
  const handleToggleConsoleOutput = (enabled: boolean) => {
    try {
      setConsoleOutput(enabled);
      setConsoleOutputState(enabled);
      sonnerToast(enabled ? "Console output enabled" : "Console output disabled", {
        description: enabled 
          ? "Logs will now be printed to the browser console" 
          : "Logs will no longer be printed to the browser console"
      });
    } catch (error) {
      console.error('Error toggling console output:', error);
      sonnerToast("Error toggling console output", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };
  
  // Handle enabling console interception
  const [interceptorEnabled, setInterceptorEnabled] = useState(false);
  
  const handleEnableInterception = () => {
    try {
      if (!interceptorEnabled) {
        installConsoleInterceptor();
        setInterceptorEnabled(true);
        sonnerToast("Console interception enabled", {
          description: "Console logs will now be captured and displayed in the debug console"
        });
        logger.info('Console interception enabled by user');
      } else {
        sonnerToast("Console interception already enabled", {
          description: "Console logs are already being captured"
        });
      }
    } catch (error) {
      console.error('Error enabling console interception:', error);
      sonnerToast("Error enabling console interception", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };

  // Handle retrying database connection
  const handleRetryDatabase = async () => {
    try {
      setIsDbOperationInProgress(true);
      logger.info('Manually retrying database connection');
      
      // Clear any existing database error
      clearDatabaseError();
      
      // Attempt to get the database, which will trigger creation if needed
      await getDatabase();
      
      sonnerToast.success("Database connection restored", {
        description: "Successfully reconnected to the database"
      });
      
      logger.info('Database connection successfully restored');
    } catch (error) {
      logger.error('Failed to retry database connection', error);
      
      // Don't set error directly here, it will be caught by the event handler
      sonnerToast.error("Database connection failed", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    } finally {
      setIsDbOperationInProgress(false);
    }
  };
  
  // Handle clearing database (wipe all data)
  const handleClearDatabase = async () => {
    try {
      if (!confirm("Are you sure you want to clear the database? This will delete ALL data and cannot be undone.")) {
        return;
      }
      
      setIsDbOperationInProgress(true);
      logger.info('Manually clearing database');
      
      // Clear any existing database error
      clearDatabaseError();
      
      // Clean up the database
      await cleanupDatabase();
      
      sonnerToast.success("Database cleared", {
        description: "All database data has been removed. The application will attempt to recreate the database."
      });
      
      logger.info('Database successfully cleared');
      
      // Attempt to recreate the database
      try {
        await getDatabase();
        logger.info('Database successfully recreated after clear');
      } catch (dbError) {
        logger.error('Failed to recreate database after clear', dbError);
        // Error will be handled by event listener
      }
    } catch (error) {
      logger.error('Failed to clear database', error);
      sonnerToast.error("Database clear failed", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    } finally {
      setIsDbOperationInProgress(false);
    }
  };
  
  // Handle downloading logs
  const handleDownloadLogs = () => {
    try {
      // Create a JSON string of the logs
      const logsJson = JSON.stringify(logs, null, 2);
      
      // Create a Blob with the JSON data
      const blob = new Blob([logsJson], { type: 'application/json' });
      
      // Create a URL for the Blob
      const url = URL.createObjectURL(blob);
      
      // Create a temporary anchor element to download the file
      const a = document.createElement('a');
      a.href = url;
      a.download = `openagents-logs-${new Date().toISOString().replace(/:/g, '-')}.json`;
      
      // Trigger the download
      document.body.appendChild(a);
      a.click();
      
      // Clean up
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      
      sonnerToast("Logs downloaded successfully", {
        description: "All log entries have been saved to a JSON file"
      });
    } catch (error) {
      console.error('Error downloading logs:', error);
      sonnerToast("Error downloading logs", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };

  // Copy log text to clipboard
  const copyLogToClipboard = (log: LogEntry) => {
    try {
      // Format the log entry as text
      const logText = `[${new Date(log.timestamp).toISOString()}] [${log.level.toUpperCase()}] [${log.module}] ${log.message}${log.details ? ' ' + JSON.stringify(log.details) : ''}`;
      
      // Copy to clipboard
      navigator.clipboard.writeText(logText);
      
      sonnerToast("Log copied to clipboard", {
        description: "Log entry has been copied to your clipboard"
      });
    } catch (error) {
      console.error('Error copying log to clipboard:', error);
      sonnerToast("Error copying log to clipboard", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };

  // Force a database error to test logging
  const triggerTestDatabaseError = async () => {
    try {
      logger.info('Testing database error handling...');
      
      // If console capture isn't enabled, prompt the user
      if (!interceptorEnabled) {
        const shouldEnable = window.confirm(
          "Console capture is not enabled. Would you like to enable it to see all logs in the debug console?"
        );
        
        if (shouldEnable) {
          handleEnableInterception();
        }
      }
      
      // Import database dynamically
      const { getDatabase, cleanupDatabase } = await import('@openagents/core/src/db/database');
      
      // Force an error by trying to access a non-existent collection
      const db = await getDatabase();
      // @ts-ignore - Intentionally causing an error for testing
      const nonExistentCollection = db.non_existent_collection;
      
      // This should not be reached
      logger.error('Test database error failed - no error was thrown');
    } catch (error) {
      logger.error('Test database error succeeded', error);
      sonnerToast("Test database error triggered", {
        description: "Check the logs for the error details"
      });
    }
  };

  // Render log level badge
  const renderLogLevelBadge = (level: LogLevel) => {
    switch (level) {
      case 'debug':
        return <Badge className="bg-blue-500 hover:bg-blue-600"><Bug className="h-3 w-3 mr-1" /> Debug</Badge>;
      case 'info':
        return <Badge className="bg-green-500 hover:bg-green-600"><Info className="h-3 w-3 mr-1" /> Info</Badge>;
      case 'warn':
        return <Badge className="bg-amber-500 hover:bg-amber-600"><FileWarning className="h-3 w-3 mr-1" /> Warning</Badge>;
      case 'error':
        return <Badge variant="destructive"><FileX className="h-3 w-3 mr-1" /> Error</Badge>;
      default:
        return <Badge>{level}</Badge>;
    }
  };

  return (
    <>
      <Card className="font-mono">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div>
            <CardTitle>Debug Console</CardTitle>
            <CardDescription>
              View application logs and diagnose issues
            </CardDescription>
          </div>
          <div className="flex space-x-2">
            <Button
              size="sm"
              onClick={refreshLogs}
              disabled={refreshing}
              className="flex items-center gap-1"
            >
              <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
              Refresh
            </Button>
            <Button
              size="sm"
              onClick={handleDownloadLogs}
              className="flex items-center gap-1"
              variant="outline"
            >
              <Download className="h-4 w-4" />
              Download
            </Button>
            <Button
              size="sm"
              onClick={handleClearLogs}
              className="flex items-center gap-1"
              variant="destructive"
            >
              <Trash className="h-4 w-4" />
              Clear
            </Button>
          </div>
        </CardHeader>

        <CardContent className="space-y-4">
          {/* Filter controls */}
          <div className="flex flex-wrap gap-4 items-center">
            <div className="flex items-center gap-2">
              <Label htmlFor="console-output" className="cursor-pointer">Console Output</Label>
              <Switch
                id="console-output"
                checked={consoleOutput}
                onCheckedChange={handleToggleConsoleOutput}
              />
            </div>
            
            <div className="flex items-center gap-2">
              <Label htmlFor="auto-refresh" className="cursor-pointer">Auto Refresh</Label>
              <Switch
                id="auto-refresh"
                checked={autoRefresh}
                onCheckedChange={setAutoRefresh}
              />
            </div>
            
            <Button
              size="sm"
              onClick={handleEnableInterception}
              disabled={interceptorEnabled}
              variant="outline"
              className="flex items-center gap-1"
            >
              <Terminal className="h-4 w-4" />
              {interceptorEnabled ? "Console Capture Active" : "Capture Console Logs"}
            </Button>
            
            <div className="flex items-center gap-2">
              <Label htmlFor="log-level">Level</Label>
              <Select value={filter} onValueChange={(value) => setFilter(value as LogLevel | 'all')}>
                <SelectTrigger className="w-32">
                  <SelectValue placeholder="All levels" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All Levels</SelectItem>
                  <SelectItem value="debug">Debug</SelectItem>
                  <SelectItem value="info">Info</SelectItem>
                  <SelectItem value="warn">Warning</SelectItem>
                  <SelectItem value="error">Error</SelectItem>
                </SelectContent>
              </Select>
            </div>
            
            <div className="flex items-center gap-2">
              <Label htmlFor="module-filter">Module</Label>
              <Select 
                value={moduleFilter} 
                onValueChange={setModuleFilter}
              >
                <SelectTrigger className="w-40">
                  <SelectValue placeholder="All modules" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All Modules</SelectItem>
                  {uniqueModules.map(module => (
                    <SelectItem key={module} value={module}>{module}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            
            <div className="flex items-center gap-2 flex-1">
              <Label htmlFor="search-filter">Search</Label>
              <Input
                id="search-filter"
                value={searchFilter}
                onChange={(e) => setSearchFilter(e.target.value)}
                placeholder="Filter logs..."
                className="flex-1"
              />
            </div>
          </div>
          
          <Separator />
          
          {/* Debug actions */}
          <div>
            <h3 className="text-sm font-medium mb-2">Debug Actions</h3>
            <div className="flex flex-wrap gap-2">
              <Button 
                size="sm" 
                variant="outline"
                onClick={triggerTestDatabaseError}
                className="flex items-center gap-1"
              >
                <Database className="h-4 w-4" />
                Test Database Error
              </Button>
              
              <Button 
                size="sm" 
                variant="outline"
                onClick={() => logger.info('Test info log triggered by user')}
                className="flex items-center gap-1"
              >
                <Info className="h-4 w-4" />
                Test Info Log
              </Button>
              
              <Button 
                size="sm" 
                variant="outline"
                onClick={() => logger.error('Test error log triggered by user')}
                className="flex items-center gap-1"
              >
                <AlertCircle className="h-4 w-4" />
                Test Error Log
              </Button>
            </div>
          </div>
          
          <Separator />
          
          {/* Info banner */}
          {!interceptorEnabled && (
            <Alert className="bg-blue-500/10 border-blue-500/50 text-blue-700 dark:text-blue-300">
              <Info className="h-4 w-4" />
              <AlertTitle>Console capture not enabled</AlertTitle>
              <AlertDescription>
                Click the "Capture Console Logs" button to start capturing all console output. 
                This will help you diagnose issues in production where the developer console is not available.
              </AlertDescription>
            </Alert>
          )}
          
          {/* Logs display */}
          <div className="border rounded-md">
            <div className="p-2 bg-secondary/20 border-b flex justify-between items-center">
              <span className="text-sm font-medium">
                Log Entries ({logs.length})
              </span>
              <Badge variant="outline" className="text-xs">
                {moduleFilter === 'all' ? 'All modules' : moduleFilter} | {filter === 'all' ? 'All levels' : filter}
              </Badge>
            </div>
            
            <ScrollArea className="h-[500px]">
              {logs.length === 0 ? (
                <div className="p-4 text-center text-muted-foreground">
                  No logs available. Try changing your filters or generating some activity.
                </div>
              ) : (
                <div className="divide-y">
                  {logs.map((log, index) => (
                    <div key={index} className="p-3 hover:bg-secondary/10">
                      <div className="flex justify-between items-start">
                        <div>
                          <div className="flex items-center gap-2 mb-1">
                            {renderLogLevelBadge(log.level)}
                            <span className="text-xs text-muted-foreground">
                              {new Date(log.timestamp).toLocaleString()}
                            </span>
                            <Badge variant="outline" className="text-xs">
                              {log.module}
                            </Badge>
                          </div>
                          <div className="text-sm whitespace-pre-wrap">
                            {log.message}
                          </div>
                          {log.details && (
                            <div className="mt-1 text-xs text-muted-foreground bg-secondary/10 p-2 rounded overflow-auto max-h-20">
                              <pre>
                                {typeof log.details === 'object' 
                                  ? JSON.stringify(log.details, null, 2) 
                                  : String(log.details)
                                }
                              </pre>
                            </div>
                          )}
                        </div>
                        <Button 
                          size="icon" 
                          variant="ghost" 
                          className="h-6 w-6"
                          onClick={() => copyLogToClipboard(log)}
                        >
                          <Copy className="h-3 w-3" />
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </ScrollArea>
          </div>
        </CardContent>
      </Card>
      
      {/* Database Management Section */}
      <Card className="mt-4">
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2">
                <Database className="h-5 w-5" />
                Database Management
              </CardTitle>
              <CardDescription>
                Manage the application database. Use these tools to diagnose and fix database issues.
              </CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {/* Display database error notification if there is an error */}
          {databaseError && (
            <DatabaseErrorNotification 
              error={databaseError} 
              onClose={clearDatabaseError}
              onRetry={handleRetryDatabase}
            />
          )}
          
          <div className="grid grid-cols-1 gap-4 mb-6">
            <div className="flex flex-col">
              <h3 className="text-sm font-medium mb-2">Database Status</h3>
              
              <div className="bg-muted/40 p-4 rounded-md mb-4">
                <div className="grid grid-cols-2 gap-x-4 gap-y-2">
                  <div className="text-sm font-medium">Status</div>
                  <div className="text-sm">
                    {databaseError ? (
                      <span className="flex items-center text-destructive">
                        <ServerCrash className="h-4 w-4 mr-1" /> Error
                      </span>
                    ) : (
                      <span className="flex items-center text-green-500">
                        <Database className="h-4 w-4 mr-1" /> Connected
                      </span>
                    )}
                  </div>
                  
                  <div className="text-sm font-medium">Environment</div>
                  <div className="text-sm">{process.env.NODE_ENV}</div>
                  
                  <div className="text-sm font-medium">Error Code</div>
                  <div className="text-sm">
                    {databaseError && typeof databaseError === 'object' && 'code' in databaseError
                      ? String(databaseError.code)
                      : '-'}
                  </div>
                </div>
              </div>
              
              <div className="flex flex-col gap-2">
                <Button 
                  onClick={handleRetryDatabase}
                  disabled={isDbOperationInProgress || !databaseError}
                  className="w-full"
                  variant="outline"
                >
                  <RotateCcw className="h-4 w-4 mr-2" />
                  {isDbOperationInProgress ? 'Connecting...' : 'Retry Database Connection'}
                </Button>
                
                <Button 
                  onClick={handleClearDatabase}
                  disabled={isDbOperationInProgress}
                  className="w-full"
                  variant="destructive"
                >
                  <Trash className="h-4 w-4 mr-2" />
                  {isDbOperationInProgress ? 'Clearing...' : 'Clear Database (Wipe All Data)'}
                </Button>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </>
  );
}