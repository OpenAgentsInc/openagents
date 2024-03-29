# Gemini API Overview

*Markdown version of [Gemini API Overview](https://ai.google.dev/docs/gemini_api_overview) retrieved March 28, 2024*

## Introduction

The Gemini API gives you access to the latest generative models from Google. Once you're familiar with the general
features available to you through the API, try a quickstart for your language of choice to start developing.

## Models

Gemini is a series of multimodal generative AI models developed by Google. Gemini models can accept text and image in
prompts, depending on what model variation you choose, and output text responses. The legacy PaLM models accept
text-only and output text responses.

- To get more detailed model information refer to the models page.
- You can also use the `list_models` method to list all the models available and then the `get_model` method to get the
  metadata for a particular model.

## Prompt Data and Design

Specific Gemini models accept both images and text data as input. This capability creates many additional possibilities
for generating content, analyzing data, and solving problems. There are some limitations and requirements to consider,
including the general input token limit for the model you are using. For information on the token limits for specific
models, see Gemini models.

### Image Requirements for Prompts

Prompts that use image data are subject to the following limitations and requirements:

- Images must be in one of the following image data MIME types:
    - PNG - `image/png`
    - JPEG - `image/jpeg`
    - WEBP - `image/webp`
    - HEIC - `image/heic`
    - HEIF - `image/heif`
- Maximum of 16 individual images
- Maximum of 4MB for the entire prompt, including images and text
- No specific limits to the number of pixels in an image; however, larger images are scaled down to fit a maximum
  resolution of 3072 x 3072 while preserving their original aspect ratio.

#### Recommendations:

- Prompts with a single image tend to yield better results.

### Prompt Design and Text Input

Creating effective prompts, or prompt engineering, is a combination of art and science. See the prompt guidelines for
guidance on how to approach prompting and the prompt 101 guide to learn about different approaches to prompting.

## Generate Content

The Gemini API lets you use both text and image data for prompting, depending on what model variation you use. For
example, you can generate text using text prompts with the gemini-pro model and use both text and image data to prompt
the gemini-pro-vision model. This section gives simple code examples of each. Refer to the generateContent API reference
for a more detailed example that covers all of the parameters.

### Text and Image Input

You can send a text prompt with an image to the gemini-pro-vision model to perform a vision related task. For example,
captioning an image or identifying what's in an image.

**Note:** You can't send a text-only prompt to the gemini-pro-vision model. Use the gemini-pro model for text-only
prompts.

#### Code Example:

```bash
curl https://generativelanguage.googleapis.com/v1/models/gemini-pro-vision:GenerateContent?key=${API_KEY} \
    -H 'Content-Type: application/json' \
    -d @<(echo'{
          "contents":[
            { "parts":[
                {"text": "Do these look store-bought or homemade?"},
                { "inlineData": {
                    "mimeType": "image/png",
                    "data": "'$(base64 -w0 cookie.png)'"
                  }
                }
              ]
            }
          ]
         }')
```

### Text Only Input

The Gemini API can also handle text-only input. This feature lets you perform natural language processing (NLP) tasks
such as text completion and summarization.

#### Code Example:

```
curl https://generativelanguage.googleapis.com/v1/models/gemini-pro:generateContent?key=$API_KEY \
-H 'Content-Type: application/json' \
-X POST \
-d '{ "contents":[
{ "parts":[{"text": "Write a story about a magic backpack"}]}
]
}'
```

## Multi-Turn Conversations (Chat)

You can use the Gemini API to build interactive chat experiences for your users. Using the chat feature of the API lets
you collect multiple rounds of questions and responses, allowing users to incrementally step toward answers or get help
with multi-part problems. This feature is ideal for applications that require ongoing communication, such as chatbots,
interactive tutors, or customer support assistants.

#### Code Example:

```
curl https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent?key=$API_KEY \
-H 'Content-Type: application/json' \
-X POST \
-d '{
"contents": [
{"role":"user",
"parts":[{
"text": "Pretend you're a snowman and stay in character for each
{"role": "model",
response."}]},
"parts":[{
"text": "Hello! It's so cold! Isn't that great?"}]},
{"role": "user",
"parts":[{
"text": "What\'s your favorite season of the year?"}]},
]
}' 2> /dev/null | grep "text"
# response example:
"text": "Winter, of course!"
```

## Streamed Responses

The Gemini API provides an additional way to receive responses from generative AI models: as a data stream. A streamed
response sends incremental pieces of data back to your application as it is generated by the model. This feature lets
you respond quickly to a user request to show progress and create a more interactive experience.

Streamed responses are an option for freeform prompting and chats with Gemini models. The following code examples show
how to request a streamed response for a prompt for each supported language:

```
curl https://generativelanguage.googleapis.com/v1/models/gemini-pro:streamGenerateContent?key=${API_KEY} \
-H 'Content-Type: application/json' \
--no-buffer \
-d '{ "contents":[
{"role": "user",
"parts":[{"text": "Write a story about a magic backpack."}]
}
]
}' > response.json
```

## Embeddings

The embedding service in the Gemini API generates state-of-the-art embeddings for words, phrases, and sentences. The
resulting embeddings can then be used for NLP tasks, such as semantic search, text classification, and clustering, among
many others. See the embeddings guide to learn what embeddings are and some key use cases for the embedding service to
help you get started.

## Next Steps

- Get started with the Google AI Studio UI using the Google AI Studio quickstart.
- Try out server-side access to the Gemini API with the quickstarts for Python, Go, or Node.js.
- Start building for the web with the Web quickstart.
- Start building for mobile apps with the Swift quickstart or the Android quickstart.
- If you're an existing Google Cloud user (or you would like to use Gemini on Vertex to take advantage of the powerful
  Google Cloud ecosystem), check out Generative AI on Vertex AI to learn more.
