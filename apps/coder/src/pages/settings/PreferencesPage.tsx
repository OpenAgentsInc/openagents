import React, { useState, useEffect } from "react";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { useSettings } from "@openagents/core";
import { toast } from "sonner";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

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
          <div className="flex items-center justify-between space-x-2">
            <Label htmlFor="confirmation-toggle" className="flex flex-col space-y-1">
              <span>Confirm Thread Deletion</span>
              <span className="font-normal text-sm text-muted-foreground">
                {confirmThreadDeletion 
                  ? "You'll be asked to confirm before deleting threads" 
                  : "Threads will be deleted instantly with an undo option"}
              </span>
            </Label>
            <Switch
              id="confirmation-toggle"
              checked={confirmThreadDeletion}
              onCheckedChange={handleToggleConfirmThreadDeletion}
              disabled={isLoading}
            />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}