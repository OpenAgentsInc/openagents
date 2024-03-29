# Welcome to Gemini 1.5 Pro API & File API Access!

We're thrilled to announce your access to the **Gemini 1.5 Pro API** and the **File API**! Your insights and the
projects you create are incredibly important to us, so please don't hesitate to share your thoughts and experiences.

## Gemini 1.5 Pro API

### Access Details

You now have API access to **Gemini 1.5 Pro** within the GCP project you've shared with us.

### Getting Started

1. **API Key**: Begin by generating an API key from your whitelisted GCP Project.
2. **Model Listing**: Utilize the `list_models` Python method or execute the following cURL command:

```
curl 'https://generativelanguage.googleapis.com/v1beta/models?key=<YOUR_API_KEY>'
```

3. **Documentation and Samples**: Dive into our [Gemini API cookbook](#) or the [Gemini API docs](#). Start with the
   Python quickstart (or any programming language you prefer) and substitute the model name
   with `gemini-1.5-pro-latest`.

### Troubleshooting Access

If you encounter access issues, confirm your API key is linked to your whitelisted Google Cloud Project
at [Google AI Studio API Key](https://aistudio.google.com/app/apikey).

## File API Overview

The **Gemini File API** simplifies file uploads for use in multimodal scenarios with the Gemini API. Check out the
notebook below for a guide on uploading images and utilizing them in a GenerateContent call.

### Getting Started with the File API

- [File API FAQ](#)
- [Quickstart Colab](#)
- [Video Colab](#)
- [Python & TS Code Samples](#)
- [REST Documentation](#)

### Known Limitations

- **Maximum Request Size**: Currently capped at 20MB. Utilize the File API for larger requests.

### Upcoming Features

- Function calling
- 'Get code' feature for Gemini 1.5 Pro in Google AI Studio
- SDK support

## Feedback and Support

Your innovative uses of our models excite us! Feel free to share your projects. For direct engagement and to help us
highlight your work, please reach out.

For any API-related bugs, issues, and feature requests, report them through the [Google AI Studio app](#).

**Happy building!**

