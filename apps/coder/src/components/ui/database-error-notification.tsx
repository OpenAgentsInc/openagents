"use client"

import * as React from "react"
import { XCircle } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "./alert"
import { Button } from "./button"
import { toast } from "./sonner"

/**
 * Error message codes and their user-friendly explanations
 */
const ERROR_EXPLANATIONS: Record<string, string> = {
  'DB9': 'Database configuration error: Using development-only settings in production.',
  'COL23': 'Collection limit reached: Too many database collections created.',
  'default': 'A database error occurred. Your data may not be saved.'
}

/**
 * Suggested actions based on error codes
 */
const ERROR_ACTIONS: Record<string, React.ReactNode> = {
  'DB9': (
    <>
      <p>Try the following:</p>
      <ol className="ml-5 mt-2 list-decimal">
        <li>Close and reopen the application</li>
        <li>Clear browser data (if using browser version)</li>
        <li>Update to the latest version</li>
      </ol>
    </>
  ),
  'COL23': (
    <>
      <p>Try the following:</p>
      <ol className="ml-5 mt-2 list-decimal">
        <li>Close and reopen the application</li>
        <li>Clear browser data (if using browser version)</li>
      </ol>
    </>
  ),
  'default': (
    <>
      <p>Try the following:</p>
      <ol className="ml-5 mt-2 list-decimal">
        <li>Restart the application</li>
        <li>Check for updates</li>
        <li>If the problem persists, check the debug console</li>
      </ol>
    </>
  )
}

export interface DatabaseErrorProps {
  error: Error | null;
  onClose?: () => void;
  onRetry?: () => void;
}

export function DatabaseErrorNotification({
  error,
  onClose,
  onRetry,
}: DatabaseErrorProps) {
  if (!error) return null;

  // Extract error code if available
  const errorCode = error && typeof error === 'object' && 'code' in error ? String(error.code) : null;
  
  // Get explanation based on error code
  const explanation = errorCode && ERROR_EXPLANATIONS[errorCode] 
    ? ERROR_EXPLANATIONS[errorCode] 
    : ERROR_EXPLANATIONS['default'];
    
  // Get action guidance based on error code
  const actionGuidance = errorCode && ERROR_ACTIONS[errorCode] 
    ? ERROR_ACTIONS[errorCode] 
    : ERROR_ACTIONS['default'];

  return (
    <Alert variant="destructive" className="mb-4">
      <XCircle className="h-4 w-4" />
      <AlertTitle>Database Error</AlertTitle>
      <AlertDescription>
        <div className="mt-2">
          <p><strong>{explanation}</strong></p>
          <p className="mt-2 text-sm text-gray-500">Error details: {error.message}</p>
          <div className="mt-3">
            {actionGuidance}
          </div>
          <div className="mt-4 flex gap-3">
            {onRetry && (
              <Button variant="outline" onClick={onRetry}>
                Retry
              </Button>
            )}
            {onClose && (
              <Button variant="ghost" onClick={onClose}>
                Dismiss
              </Button>
            )}
          </div>
        </div>
      </AlertDescription>
    </Alert>
  );
}

/**
 * Show database error as a toast notification
 */
export function showDatabaseErrorToast(error: Error) {
  // Extract error code if available
  const errorCode = error && typeof error === 'object' && 'code' in error ? String(error.code) : null;
  
  // Get explanation based on error code
  const explanation = errorCode && ERROR_EXPLANATIONS[errorCode] 
    ? ERROR_EXPLANATIONS[errorCode] 
    : ERROR_EXPLANATIONS['default'];

  toast.error(
    <div>
      <h3 className="font-medium">Database Error</h3>
      <p className="text-sm">{explanation}</p>
      <p className="text-xs text-gray-500 mt-1">Error details: {error.message}</p>
    </div>,
    {
      duration: 10000, // Show for 10 seconds
      action: {
        label: "View Details",
        onClick: () => {
          // Navigate to debug page
          window.location.href = '/settings/debug';
        }
      }
    }
  );
}