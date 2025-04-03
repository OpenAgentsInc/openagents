import React, { useState, useEffect } from "react";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { useSettings } from "@openagents/core";
import { toast } from "sonner";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger } from "@/components/ui/alert-dialog";
import { cleanupDatabase } from "@openagents/core/src/db/database";

export default function PreferencesPage() {
  const { getPreference, setPreference } = useSettings();
  const [confirmThreadDeletion, setConfirmThreadDeletion] = useState(true);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadPreferences = async () => {
      try {
        setIsLoading(true);
        const shouldConfirm = await getPreference("confirmThreadDeletion", true);
        setConfirmThreadDeletion(shouldConfirm);
      } catch (error) {
        console.error("Error loading preferences:", error);
        toast.error("Failed to load preferences");
      } finally {
        setIsLoading(false);
      }
    };

    loadPreferences();
  }, [getPreference]);

  const handleToggleConfirmThreadDeletion = async (checked: boolean) => {
    try {
      setConfirmThreadDeletion(checked);
      await setPreference("confirmThreadDeletion", checked);

      toast.success(
        checked ? "Thread deletion confirmation enabled" : "Thread deletion confirmation disabled",
        {
          description: checked
            ? "You will be asked to confirm before deleting threads"
            : "Threads will be deleted instantly with an undo option"
        }
      );
    } catch (error) {
      console.error("Error saving preference:", error);
      toast.error("Failed to save preference");
      // Revert UI state if save fails
      setConfirmThreadDeletion(!checked);
    }
  };

  const handleResetDatabase = async () => {
    try {
      await cleanupDatabase();
      toast.success("Database reset successfully", {
        description: "The application will now reload to apply changes",
      });
      // Give time for the toast to be shown
      setTimeout(() => {
        window.location.reload();
      }, 1500);
    } catch (error) {
      console.error("Error resetting database:", error);
      toast.error("Failed to reset database", {
        description: "Please try restarting the application"
      });
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium">Preferences</h3>
        <p className="text-sm text-muted-foreground">
          Customize your application experience
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Thread Management</CardTitle>
          <CardDescription>
            Configure how threads are managed in the application
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Separator className="my-4" />
          <div className="flex items-center space-x-4">
            <div className="flex-1">
              <Label htmlFor="confirmation-toggle">
                <div className="space-y-1">
                  <div>Confirm Thread Deletion</div>
                  <div className="font-normal text-sm text-muted-foreground">
                    {confirmThreadDeletion
                      ? "You'll be asked to confirm before deleting threads"
                      : "Threads will be deleted instantly with an undo option"}
                  </div>
                </div>
              </Label>
            </div>
            <Switch
              id="confirmation-toggle"
              checked={confirmThreadDeletion}
              onCheckedChange={handleToggleConfirmThreadDeletion}
              disabled={isLoading}
            />
          </div>
        </CardContent>
      </Card>
      
      <Card>
        <CardHeader>
          <CardTitle>Database Management</CardTitle>
          <CardDescription>
            Reset the application database to resolve issues
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Separator className="my-4" />
          <div className="flex flex-col space-y-4">
            <div>
              <div className="font-medium">Reset Database</div>
              <div className="text-sm text-muted-foreground">
                This will reset the entire database, clearing all threads, messages, and settings. 
                Use this option if you're experiencing database-related issues.
              </div>
            </div>
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button variant="destructive">Reset Database</Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Are you absolutely sure?</AlertDialogTitle>
                  <AlertDialogDescription>
                    This action will reset the entire database, removing all threads, messages, and settings.
                    This action cannot be undone. The application will reload after the reset.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction onClick={handleResetDatabase}>Reset Database</AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
