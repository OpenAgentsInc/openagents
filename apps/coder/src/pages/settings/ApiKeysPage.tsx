import React, { useState, useEffect } from "react";
import { useSettings } from "@openagents/core";
import {
  Trash2,
  Plus,
  Eye,
  EyeOff,
  KeyRound,
  ShieldCheck
} from "lucide-react";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  CardFooter,
  Input,
  Alert,
  AlertDescription,
  AlertTitle,
  Separator,
} from "@/components/ui";

export default function ApiKeysPage() {
  const {
    setApiKey,
    getApiKey,
    deleteApiKey,
  } = useSettings();

  // Only include Anthropic and OpenRouter
  const providers = ["anthropic", "openrouter", "google"];

  // API keys state
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [keyInputs, setKeyInputs] = useState<Record<string, string>>({});
  const [isLoading, setIsLoading] = useState(true);

  // Load API keys when component mounts
  useEffect(() => {
    const loadApiKeys = async () => {
      try {
        setIsLoading(true);
        const keys: Record<string, string> = {};
        for (const provider of providers) {
          const key = await getApiKey(provider);
          if (key) {
            keys[provider] = key;
          }
        }
        setApiKeys(keys);
      } catch (error) {
        console.error("Error loading API keys:", error);
      } finally {
        setIsLoading(false);
      }
    };

    loadApiKeys();
  }, [getApiKey]);

  // Handle API key changes
  const handleApiKeyChange = (provider: string, value: string) => {
    setKeyInputs(prev => ({ ...prev, [provider]: value }));
  };

  // Save API key
  const handleSaveApiKey = async (provider: string) => {
    const key = keyInputs[provider];
    if (key) {
      await setApiKey(provider, key);
      setApiKeys(prev => ({ ...prev, [provider]: key }));
      setKeyInputs(prev => ({ ...prev, [provider]: "" }));

      // Dispatch event to notify about API key change
      try {
        window.dispatchEvent(new CustomEvent('api-key-changed', {
          detail: { provider }
        }));
        console.log(`Dispatched api-key-changed event for ${provider}`);
      } catch (eventError) {
        console.warn("Error dispatching api-key-changed event:", eventError);
      }
    }
  };

  // Delete API key
  const handleDeleteApiKey = async (provider: string) => {
    await deleteApiKey(provider);
    setApiKeys(prev => {
      const updated = { ...prev };
      delete updated[provider];
      return updated;
    });

    // Dispatch event to notify about API key deletion
    try {
      window.dispatchEvent(new CustomEvent('api-key-changed', {
        detail: { provider, deleted: true }
      }));
      console.log(`Dispatched api-key-changed event for ${provider} (deleted)`);
    } catch (eventError) {
      console.warn("Error dispatching api-key-changed event:", eventError);
    }
  };

  // Toggle API key visibility
  const toggleKeyVisibility = (provider: string) => {
    setShowKeys(prev => ({ ...prev, [provider]: !prev[provider] }));
  };

  // Get provider display name
  const getProviderDisplayName = (provider: string) => {
    if (provider === "openrouter") return "OpenRouter";
    return provider.charAt(0).toUpperCase() + provider.slice(1);
  };

  // Get provider description
  const getProviderDescription = (provider: string) => {
    switch (provider) {
      case "anthropic":
        return "Anthropic provides Claude models with exceptional reasoning capabilities.";
      case "openrouter":
        return "OpenRouter provides access to many AI models from different providers.";
      default:
        return "";
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full font-mono">
        <p>Loading API keys...</p>
      </div>
    );
  }

  return (
    <Card className="font-mono">
      <CardHeader>
        <CardTitle className="flex items-center">
          <KeyRound className="mr-2 h-5 w-5" />
          API Keys
        </CardTitle>
        <CardDescription>
          Manage your API keys for different model providers
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {providers.map((provider, index) => (
          <div key={provider} className="space-y-4">
            {/* Provider header */}
            <div className="space-y-2">
              <h3 className="text-lg font-medium">{getProviderDisplayName(provider)} API</h3>
              <p className="text-sm text-muted-foreground">
                {getProviderDescription(provider)}
              </p>
            </div>

            {/* API Key Management */}
            <div className="space-y-4">
              {apiKeys[provider] ? (
                <div className="space-y-4">
                  <div className="flex items-center space-x-2">
                    <Input
                      type={showKeys[provider] ? "text" : "password"}
                      value={apiKeys[provider]}
                      readOnly
                      className="font-mono"
                    />
                    <Button
                      variant="outline"
                      size="icon"
                      onClick={() => toggleKeyVisibility(provider)}
                    >
                      {showKeys[provider] ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                    </Button>
                    <Button
                      variant="destructive"
                      size="icon"
                      onClick={() => handleDeleteApiKey(provider)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              ) : (
                <Alert>
                  <AlertTitle className="font-mono">No API key set</AlertTitle>
                  <AlertDescription className="font-mono">
                    You haven't set an API key for {provider} yet. Add one below to use {provider} models.
                  </AlertDescription>
                </Alert>
              )}

              <div className="flex items-center space-x-2">
                <Input
                  type="password"
                  placeholder={`Enter your ${provider} API key`}
                  value={keyInputs[provider] || ""}
                  onChange={(e) => handleApiKeyChange(provider, e.target.value)}
                  className="font-mono"
                />
                <Button onClick={() => handleSaveApiKey(provider)}>
                  <Plus className="h-4 w-4 mr-2" />
                  Save Key
                </Button>
              </div>
            </div>

            {/* Add separator between providers, except after the last one */}
            {index < providers.length - 1 && (
              <Separator className="my-4" />
            )}
          </div>
        ))}
      </CardContent>
      <CardFooter className="flex flex-col space-y-4 text-xs text-muted-foreground">
        <div className="w-full flex items-center">
          <ShieldCheck className="h-4 w-4 mr-2 text-green-500" />
          <span>Your API keys are stored securely in your browser's local database.</span>
        </div>
      </CardFooter>
    </Card>
  );
}
