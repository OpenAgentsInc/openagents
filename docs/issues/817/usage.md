# Model Selection Usage Guide

## Basic Usage

### Selecting a Model

1. **From the Chat Interface**:
   - In the header of the chat interface, there's a dropdown showing the currently selected model
   - Click on the dropdown to see a list of available models
   - Search for a specific model using the search box
   - Select any model to switch to it for your current conversation

2. **Setting a Default Model**:
   - Navigate to Settings → Models & API Keys
   - In the "Default Model" section, select your preferred default model
   - This model will be used for all new conversations

### Managing API Keys

1. **Adding an API Key**:
   - Navigate to Settings → Models & API Keys
   - Select the provider tab (OpenRouter, Anthropic, or Groq)
   - Enter your API key in the input field
   - Click "Save Key" to store it securely

2. **Viewing an API Key**:
   - Navigate to Settings → Models & API Keys
   - Select the provider tab
   - Your saved API key will be displayed as a password field
   - Click the eye icon to toggle between showing and hiding the key

3. **Deleting an API Key**:
   - Navigate to Settings → Models & API Keys
   - Select the provider tab
   - Click the trash icon next to the API key to delete it

## Advanced Usage

### Free vs Pro Models

- Models are labeled with either a "PRO" badge or no badge (Free)
- Free models:
  - Can be used without an API key
  - May have limitations in capabilities or usage quotas
  - Great for testing or casual use

- Pro models:
  - Require a valid API key for the respective provider
  - Offer enhanced capabilities, longer context windows, or better performance
  - Charges are applied to your account with the respective provider

### Understanding Model Capabilities

Each model in the selection dropdown and settings page displays:

1. **Name**: The model's identifier
2. **Description**: A brief explanation of the model's capabilities and strengths
3. **Context Length**: The maximum number of tokens the model can process at once
4. **Tools Support**: Whether the model supports function calling/tools

### Provider-Specific Information

- **OpenRouter**:
  - Provides access to a variety of models from different providers
  - Requires an OpenRouter API key
  - Offers both free and paid models

- **Anthropic**:
  - Offers Claude models with exceptional reasoning capabilities
  - Requires an Anthropic API key for all models
  - Known for strong safety features and following instructions precisely

- **Groq**:
  - Provides ultra-fast inference for various open models
  - Requires a Groq API key
  - Known for exceptional speed and competitive pricing

## Troubleshooting

### Model Not Working

1. **Check API Key**:
   - Ensure you've entered a valid API key for the model's provider
   - API keys are sensitive to whitespace, so avoid extra spaces

2. **Provider Status**:
   - Check the provider's status page for any outages
   - Some providers may have usage limits that you've reached

3. **Model Compatibility**:
   - Some models have specific requirements or limitations
   - Check if the model supports the features you're trying to use

### API Key Issues

1. **Invalid Key Error**:
   - Double-check that the key is copied correctly
   - Generate a new key from the provider if necessary

2. **Missing Key Warning**:
   - If you see a warning about a missing API key, navigate to Settings → Models & API Keys
   - Add the required key for the provider