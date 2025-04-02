import React, { useState, useEffect } from "react";
import { 
  Card, 
  CardHeader, 
  CardTitle, 
  CardDescription, 
  CardContent,
  Button,
  Input,
  Alert,
  AlertTitle,
  AlertDescription
} from "@/components/ui";
import { CheckCircle, XCircle, RefreshCw, Terminal } from "lucide-react";

export default function LocalModelsPage() {
  // Removed Ollama tabs and states to focus only on LMStudio
  const [lmStudioStatus, setLmStudioStatus] = useState<'checking' | 'running' | 'not-running'>('checking');
  const [lmStudioModels, setLmStudioModels] = useState<string[]>([]);
  const [lmStudioUrl, setLmStudioUrl] = useState("http://localhost:1234");
  const [refreshing, setRefreshing] = useState(false);
  const [lmStudioError, setLmStudioError] = useState<string | null>(null);

  // Removed Ollama status check function

  // Check LMStudio status
  const checkLmStudioStatus = async () => {
    try {
      // Reset error state
      setLmStudioError(null);
      setLmStudioStatus('checking');
      console.log(`Checking LMStudio status at: ${lmStudioUrl}/v1/models`);
      
      // Add a timeout to the fetch to prevent hanging
      const controller = new AbortController();
      const timeoutId = setTimeout(() => {
        controller.abort();
        setLmStudioError("Connection timed out. Is LMStudio running?");
      }, 5000);
      
      // Use our server proxy to avoid CORS issues
      const proxyUrl = `/api/proxy/lmstudio/models?url=${encodeURIComponent(`${lmStudioUrl}/v1/models`)}`;
      console.log("Using proxy URL:", proxyUrl);
      
      const response = await fetch(proxyUrl, {
        method: 'GET',
        headers: {
          'Accept': 'application/json',
        },
        signal: controller.signal
      });
      
      clearTimeout(timeoutId);
      
      // Log the full response for debugging
      console.log("LMStudio response status:", response.status);
      console.log("LMStudio response OK:", response.ok);
      
      if (response.ok) {
        const data = await response.json();
        console.log("LMStudio models:", data);
        
        // Extract model names from the response - handle various response formats
        let modelNames: string[] = [];
        
        console.log("Raw LMStudio model data:", JSON.stringify(data, null, 2));
        
        if (data && data.data && Array.isArray(data.data)) {
          // Standard OpenAI format
          console.log("Found standard OpenAI format model data");
          modelNames = data.data.map((model: any) => model.id || "Unknown model");
        } else if (data && Array.isArray(data)) {
          // Possible alternative format
          console.log("Found array format model data");
          modelNames = data.map((model: any) => {
            if (typeof model === 'string') return model;
            return model.id || model.name || model.model || "Unknown model";
          });
        } else if (data && typeof data === 'object') {
          // Try to extract any properties that might contain model information
          console.log("Found object format model data, attempting to extract");
          if (data.models && Array.isArray(data.models)) {
            modelNames = data.models.map((model: any) => {
              if (typeof model === 'string') return model;
              return model.id || model.name || model.model || "Unknown model";
            });
          } else {
            // Last resort: try to get any string properties from the object
            const possibleModels = Object.entries(data)
              .filter(([key, value]) => typeof value === 'string' && 
                ['id', 'name', 'model'].includes(key.toLowerCase()))
              .map(([_, value]) => value);
              
            if (possibleModels.length > 0) {
              modelNames = possibleModels;
            }
          }
        }
        
        console.log("Extracted model names:", modelNames);
        
        setLmStudioModels(modelNames);
        setLmStudioStatus('running');
      } else {
        let errorMessage = `Server responded with status ${response.status}`;
        
        // Make a copy of the response for reading the body, as we can only read it once
        const clonedResponse = response.clone();
        
        try {
          const errorData = await clonedResponse.json();
          console.log("LMStudio error response:", errorData);
          if (errorData.error) {
            errorMessage = errorData.error;
          }
          if (errorData.details) {
            errorMessage += `: ${errorData.details}`;
          }
        } catch (e) {
          console.error("Error parsing error response as JSON:", e);
          errorMessage = "Failed to connect to LMStudio server";
        }
        
        setLmStudioError(errorMessage);
        setLmStudioStatus('not-running');
      }
    } catch (error: any) {
      console.error("Error checking LMStudio status:", error);
      
      // Set a more helpful error message
      if (error.name === 'AbortError') {
        setLmStudioError("Connection timed out. Is LMStudio running?");
      } else if (error.message && error.message.includes('NetworkError')) {
        setLmStudioError("Network error. Is LMStudio running with the local server enabled?");
      } else if (error.message && error.message.includes('CORS')) {
        setLmStudioError("CORS error. Try restarting LMStudio with CORS enabled.");
      } else {
        setLmStudioError(error.message || "Unknown error connecting to LMStudio");
      }
      
      setLmStudioStatus('not-running');
    }
  };

  // Handle refresh button click
  const handleRefresh = async () => {
    setRefreshing(true);
    await checkLmStudioStatus();
    setRefreshing(false);
  };

  // Check status on component mount
  useEffect(() => {
    checkLmStudioStatus();
  }, []);

  return (
    <>
      <Card className="font-mono">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div>
            <CardTitle>LMStudio Configuration</CardTitle>
            <CardDescription>
              Configure your local LMStudio models
            </CardDescription>
          </div>
          <Button 
            variant="outline" 
            size="icon" 
            onClick={handleRefresh}
            disabled={refreshing}
          >
            <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
          </Button>
        </CardHeader>
        
        <CardContent className="space-y-4">
          {/* Connection Status */}
          <div className="space-y-2">
            <div className="text-sm font-medium">Connection Status</div>
            <div className="flex items-center">
              {lmStudioStatus === 'checking' ? (
                <RefreshCw className="h-4 w-4 mr-2 animate-spin text-yellow-500" />
              ) : lmStudioStatus === 'running' ? (
                <CheckCircle className="h-4 w-4 mr-2 text-green-500" />
              ) : (
                <XCircle className="h-4 w-4 mr-2 text-red-500" />
              )}
              <div>
                <span>
                  {lmStudioStatus === 'checking' 
                    ? 'Checking LMStudio...' 
                    : lmStudioStatus === 'running' 
                      ? 'LMStudio is running' 
                      : 'LMStudio is not running'
                  }
                </span>
                {lmStudioError && (
                  <div className="text-sm text-red-500 mt-1">
                    Error: {lmStudioError}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Server URL */}
          <div className="space-y-2">
            <div className="text-sm font-medium">LMStudio Server URL</div>
            <div className="flex gap-2">
              <Input
                value={lmStudioUrl}
                onChange={(e) => setLmStudioUrl(e.target.value)}
                placeholder="http://localhost:1234"
              />
              <Button 
                onClick={() => {
                  console.log("Connect button clicked for LMStudio");
                  checkLmStudioStatus();
                }}
                disabled={refreshing}
              >
                Connect
              </Button>
            </div>
          </div>

          {/* Available Models */}
          <div className="space-y-2">
            <div className="text-sm font-medium">Available Models</div>
            {lmStudioStatus === 'running' ? (
              lmStudioModels.length > 0 ? (
                <div className="border rounded-md divide-y">
                  {lmStudioModels.map((model, i) => (
                    <div key={i} className="p-2 text-sm flex items-center">
                      <CheckCircle className="h-3 w-3 mr-2 text-green-500" />
                      {model}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="text-sm text-muted-foreground">No models detected in LMStudio server</div>
              )
            ) : (
              <div className="text-sm text-muted-foreground">Connect to LMStudio to see available models</div>
            )}
          </div>

          {/* Installation Guide */}
          <Alert>
            <Terminal className="h-4 w-4" />
            <AlertTitle>Installation Guide</AlertTitle>
            <AlertDescription>
              <p className="mb-2">To use LMStudio, follow these steps:</p>
              <ol className="list-decimal pl-4 space-y-1 text-sm">
                <li>Download LMStudio from <a href="https://lmstudio.ai" target="_blank" rel="noopener noreferrer" className="text-blue-500 hover:underline">lmstudio.ai</a></li>
                <li>Install and start LMStudio</li>
                <li>In LMStudio, go to the "Local Server" tab and click "Start server"</li>
                <li>Download models in LMStudio and add them to your local inference server</li>
              </ol>
            </AlertDescription>
          </Alert>
        </CardContent>
      </Card>
    </>
  );
}