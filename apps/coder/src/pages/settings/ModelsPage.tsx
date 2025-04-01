import React, { useEffect, useState } from "react";
import { useSettings, models } from "@openagents/core";
import { Trash2, Plus, Eye, EyeOff } from "lucide-react";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Input,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui";
import { ModelSelect } from "@/components/ui/model-select";

// Group models by provider for better organization
const providerGroups = models.reduce((acc, model) => {
  const provider = model.provider;
  if (!acc[provider]) {
    acc[provider] = [];
  }
  acc[provider].push(model);
  return acc;
}, {} as Record<string, typeof models>);

// Get unique provider names
const providers = Object.keys(providerGroups);

interface Settings {
  defaultModel: string;
  // Add other settings properties as needed
}

export default function ModelsPage() {
  const { settings, isLoading, setApiKey, getApiKey, deleteApiKey, updateSettings, clearSettingsCache, resetSettings } = useSettings();
  const [defaultModelId, setDefaultModelId] = useState("");
  const [currentProvider, setCurrentProvider] = useState(providers[0] || "");
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [keyInputs, setKeyInputs] = useState<Record<string, string>>({});

  // Load settings when component mounts
  useEffect(() => {
    if (settings) {
      console.log("ModelsPage: Loading settings, default model =", settings.defaultModel);

      // Verify the model exists in our list
      let modelToUse = settings.defaultModel;
      if (modelToUse) {
        const modelExists = models.some(model => model.id === modelToUse);
        if (!modelExists) {
          console.warn(`Model ${modelToUse} not found in models list`);
          modelToUse = models[0]?.id || "";
        }
      } else {
        modelToUse = models[0]?.id || "";
      }

      setDefaultModelId(modelToUse);

      // Load API keys for all providers
      const loadApiKeys = async () => {
        const keys: Record<string, string> = {};
        for (const provider of providers) {
          const key = await getApiKey(provider);
          if (key) {
            keys[provider] = key;
          }
        }
        setApiKeys(keys);
      };

      loadApiKeys();
    }
  }, [settings, getApiKey]);

  // Handle default model change
  const handleDefaultModelChange = async (modelId: string) => {
    try {
      // Check if the model exists in the list
      const modelExists = models.some(model => model.id === modelId);
      if (!modelExists) {
        console.error(`Model ${modelId} not found in models list`);
        return;
      }

      // Update UI immediately to give user feedback
      setDefaultModelId(modelId);

      console.log(`Updating default model to: ${modelId}`);

      // Simple approach - create a clean object with only the field we're updating
      const cleanUpdate = { defaultModel: modelId };
      const result = await updateSettings(cleanUpdate) as Settings;
      console.log("Settings update result:", JSON.stringify(result));

      // Verify the update by checking the returned result
      if (result.defaultModel !== modelId) {
        console.warn(`Update verification warning: expected ${modelId}, got ${result.defaultModel}`);

        // Try once more but do not force page reload
        try {
          // Try again with a clearer approach
          await clearSettingsCache();
          // Create a clean object again for the second attempt
          const secondCleanUpdate = { defaultModel: modelId };
          const secondResult = await updateSettings(secondCleanUpdate) as Settings;

          if (secondResult.defaultModel === modelId) {
            console.log("Second update attempt succeeded");
          } else {
            console.warn("Second update attempt did not match expected model, but continuing without reload");
            // Still use the model ID from UI update to maintain user experience
          }
        } catch (retryError) {
          console.error("Retry update failed:", retryError);
          // Continue without reloading page
        }
      } else {
        console.log("Default model updated successfully and verified");
      }
    } catch (error) {
      console.error("Error updating default model:", error);

      // Handle error without page reload
      // Just show an error message and keep the UI state
      alert("There was an error saving your model preference. The model will be used for this session only.");

      // No reload, no localStorage.clear()
      // Just maintain the UI state with the selected model
    }
  };

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
  };

  // Toggle API key visibility
  const toggleKeyVisibility = (provider: string) => {
    setShowKeys(prev => ({ ...prev, [provider]: !prev[provider] }));
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full font-mono">
        <p>Loading settings...</p>
      </div>
    );
  }

  return (
    <>
      {/* Default Model Selection */}
        <Card className="font-mono">
          <CardHeader>
            <CardTitle>Default Model</CardTitle>
            <CardDescription>
              Choose the default AI model to use for new conversations
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <ModelSelect
                value={defaultModelId}
                onChange={handleDefaultModelChange}
                placeholder="Select default model"
              />

              <div className="flex justify-end">
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={async () => {
                    if (confirm("Reset all settings to default? This will clear your saved API keys.")) {
                      try {
                        const defaultSettings = await resetSettings();

                        if (defaultSettings) {
                          // Update UI to reflect new settings
                          setDefaultModelId(defaultSettings.defaultModel || 'qwen-qwq-32b');
                          setApiKeys({});

                          alert("Settings reset successfully.");

                          // Load API keys (there should be none after reset)
                          const loadApiKeys = async () => {
                            const keys: Record<string, string> = {};
                            for (const provider of providers) {
                              const key = await getApiKey(provider);
                              if (key) {
                                keys[provider] = key;
                              }
                            }
                            setApiKeys(keys);
                          };

                          await loadApiKeys();
                        } else {
                          // Fallback if reset returns null
                          console.error("Settings reset returned null result");
                          alert("Settings reset partially completed. You may need to refresh the page.");
                        }
                      } catch (error) {
                        console.error("Failed to reset settings:", error);
                        alert("There was a problem resetting settings. Please try again later.");
                      }
                    }
                  }}
                >
                  Reset All Settings
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* API Keys */}
        <Card className="font-mono">
          <CardHeader>
            <CardTitle>API Keys</CardTitle>
            <CardDescription>
              Manage your API keys for different model providers
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <Tabs value={currentProvider} onValueChange={setCurrentProvider}>
              <TabsList className="grid font-mono" style={{ gridTemplateColumns: `repeat(${providers.length}, 1fr)` }}>
                {providers.map(provider => (
                  <TabsTrigger key={provider} value={provider} className="capitalize font-mono">
                    {provider}
                  </TabsTrigger>
                ))}
              </TabsList>

              {providers.map(provider => (
                <TabsContent key={provider} value={provider} className="space-y-4">
                  {/* Provider Info */}
                  <div className="space-y-2">
                    <h3 className="text-lg font-medium capitalize">{provider} Models</h3>
                    <p className="text-sm text-muted-foreground">
                      {provider === "anthropic" && "Anthropic provides Claude models with exceptional reasoning capabilities."}
                      {provider === "openrouter" && "OpenRouter provides access to many AI models from different providers."}
                      {provider === "groq" && "Groq offers ultra-fast inference for various open models."}
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

                  {/* Available Models */}
                  <div className="mt-6">
                    <h4 className="text-sm font-medium mb-2">Available Models:</h4>
                    <div className="space-y-2">
                      {providerGroups[provider].map(model => (
                        <div key={model.id} className="p-3 border rounded-md font-mono">
                          <div className="flex items-center justify-between">
                            <span className="font-medium">{model.name}</span>
                            {model.plan === "pro" && (
                              <span className="text-xs rounded bg-yellow-500/10 text-yellow-500 px-1.5 py-0.5">
                                PRO
                              </span>
                            )}
                          </div>
                          {model.shortDescription && (
                            <p className="text-sm text-muted-foreground mt-1">
                              {model.shortDescription}
                            </p>
                          )}
                          <div className="flex items-center text-xs text-muted-foreground mt-2 space-x-4">
                            <span>Context: {(model.context_length / 1000).toFixed(0)}k tokens</span>
                            {model.supportsTools !== undefined && (
                              <span>
                                {model.supportsTools ? "✓ Supports tools" : "✗ No tool support"}
                              </span>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                </TabsContent>
              ))}
            </Tabs>
          </CardContent>
          <CardFooter className="text-xs text-muted-foreground">
            Your API keys are stored securely in your browser's local database.
          </CardFooter>
        </Card>
    </>
  );
}
