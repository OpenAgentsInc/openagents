# File API Frequently Asked Questions (FAQ)

## Introduction to File API

**What is the File API and why do I need this?**
The File API is designed for developers to upload files, enabling the Gemini API to leverage them in multimodal
scenarios, such as attaching images to prompts. This method is stable and reliable for enhancing your applications with
multimodal capabilities using the Gemini API.

## How It Works

**How does this work?**
The File API facilitates multimodal support in Gemini 1.5 Pro, allowing the upload of files and their subsequent
reference within your Gemini API prompts. Files can be attached to your prompts for up to 48 hours, providing the
flexibility to reuse the same file multiple times within this period without the need for re-uploading.

## Supported File Types

**Which file types are supported today?**

- **Images:** `image/png`, `image/jpeg`, `image/webp`, `image/heic`, `image/heif`
- **Video (as extracted frames only, no audio support at this time):
  ** `image/png`, `image/jpeg`, `image/webp`, `image/heic`, `image/heif`

## Integration with Gemini API Prompts

**How does this work with my Gemini API prompt?**
Upon uploading a file, you will receive a unique ID (formatted as a URI under `files/file-id`), which can then be passed
as a reference within your Gemini API prompt.

## Before File API

**Couldn’t I upload files before?**
Previously, files could be base64 encoded and embedded within a Gemini API request. However, this method was prone to
errors and limited to 20MB. The File API now allows for more reliable uploads of up to 20GB per project (2GB per file).

## Video Upload Capabilities

**How many minutes of video can I upload via the File API?**
You can upload 3,600 frames (images), equivalent to 1 hour of video, through frame extraction at a rate of 1 frame per
second.

## SDK Integration

**Will this appear in the Gemini SDK?**
Integration of the File API into the Gemini SDK is in progress. For now, the REST API is available, with example Colab
notebooks provided for both images and videos.

## Authentication and Security

**How will calls to the File API be authenticated and secured?**
File API uploads utilize the Gemini API key for authentication. It's essential to keep your API key confidential and
follow Google's best practices for securing API keys.

## Comparison to Drive and GCS for File Storage

**How does this compare to Drive and GCS for file storage?**

| Feature                             | Drive Storage                                        | GCS                                                  | Gemini File API                        |
|-------------------------------------|------------------------------------------------------|------------------------------------------------------|----------------------------------------|
| Automatically delete after 48 hours | No                                                   | No                                                   | Yes                                    |
| Authentication                      | Two-step OAuth & requires additional developer setup | Two-step OAuth & requires additional developer setup | Gemini API Key                         |
| SDK Support                         | Other SDK                                            | Vertex SDK                                           | (Coming soon) Built into Gemini SDK    |
| Download your upload                | Supported                                            | Supported                                            | Not supported (only kept for 48 hours) |

## Comparison to Vertex API

**How does this compare to uploading files in the Vertex API?**

| Feature            | Vertex API | Developer API |
|--------------------|------------|---------------|
| File URIs accepted | GCS URIs   | File API URIs |

## Support for GCS URL Paths

**Will it support GCS: URL paths?**
While Vertex AI supports GCS, the Gemini API currently does not support GCS URL paths.

## Availability in the EU

**Is this API supported in the EU?**
The Gemini API, Google AI Studio, and the File API are not available in the EU at present. Vertex AI is available in the
EU.

## Monitoring File API Usage

**How can I see usage of the File API?**
There is no dedicated dashboard for the File API. However, uploaded files can be programmatically listed using the
ListFiles endpoint.

## Usage Limits and Increases

**Can I request usage limit increases beyond 20GB/project and 2GB/file?**
We are developing a process for requesting limit increases, with further updates expected in the coming weeks.
