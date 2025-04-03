"use client"

import * as React from "react"
import { AlertCircle, RefreshCw } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "./alert"
import { Button } from "./button"
import { toast } from "sonner"

export interface NetworkErrorProps {
  onRetry?: () => void;
}

export function NetworkErrorNotification({
  onRetry,
}: NetworkErrorProps) {
  return (
    <Alert variant="destructive" className="mb-4">
      <AlertCircle className="h-4 w-4" />
      <AlertTitle>Network Connection Error</AlertTitle>
      <AlertDescription>
        <div className="mt-2">
          <p><strong>Unable to connect to the AI service.</strong></p>
          <p className="mt-2 text-sm text-gray-500">This could be due to your internet connection or the local API server.</p>
          <div className="mt-3">
            <p>Try the following:</p>
            <ol className="ml-5 mt-2 list-decimal">
              <li>Check your internet connection</li>
              <li>Restart the application</li>
              <li>If the problem persists, check the debug console in Settings</li>
            </ol>
          </div>
          <div className="mt-4 flex gap-3">
            {onRetry && (
              <Button variant="outline" onClick={onRetry}>
                <RefreshCw className="mr-2 h-4 w-4" />
                Retry Connection
              </Button>
            )}
          </div>
        </div>
      </AlertDescription>
    </Alert>
  );
}

/**
 * Show network error as a toast notification
 */
export function showNetworkErrorToast(onRetry?: () => void) {
  toast.error(
    <div>
      <h3 className="font-medium">Network Connection Error</h3>
      <p className="text-sm">Unable to connect to the AI service.</p>
      <p className="text-xs text-gray-500 mt-1">Check your internet connection or restart the app.</p>
    </div>,
    {
      duration: 10000, // Show for 10 seconds
      action: {
        label: "Debug Console",
        onClick: () => {
          // Navigate to debug page
          window.location.href = '/settings/debug';
        }
      }
    }
  );
}