# File Resource API Documentation

This documentation outlines the structure and details of the `File` resource when it's uploaded to the API, including
its JSON representation, fields, and associated methods.

## JSON Representation

The `File` resource in JSON format includes the following attributes:

```
{
"name": "string",
"displayName": "string",
"mimeType": "string",
"sizeBytes": "string",
"createTime": "string",
"updateTime": "string",
"expirationTime": "string",
"sha256Hash": "string",
"uri": "string"
}
```

## Fields Description

### `name`

- **Type:** string
- **Description:** Immutable identifier for the File resource. Consists of lowercase alphanumeric characters or
  dashes (-), up to 40 characters. Cannot start or end with a dash. A unique name is generated if left empty on
  creation.
- **Example:** `files/123-456`

### `displayName`

- **Type:** string
- **Description:** Optional. A human-readable name for the File, not exceeding 512 characters, including spaces.
- **Example:** `"Welcome Image"`

### `mimeType`

- **Type:** string
- **Description:** Output only. The MIME type of the file.

### `sizeBytes`

- **Type:** string (int64 format)
- **Description:** Output only. The size of the file in bytes.

### `createTime`

- **Type:** string (Timestamp format)
- **Description:** Output only. Timestamp of File creation, in RFC3339 UTC "Zulu" format with nanosecond resolution.
- **Examples:** `"2014-10-02T15:01:23Z"`, `"2014-10-02T15:01:23.045123456Z"`

### `updateTime`

- **Type:** string (Timestamp format)
- **Description:** Output only. Timestamp of the last File update, in RFC3339 UTC "Zulu" format with nanosecond
  resolution.
- **Examples:** Same as `createTime`.

### `expirationTime`

- **Type:** string (Timestamp format)
- **Description:** Output only. Timestamp of when the File will be deleted if scheduled to expire, in the same format
  as `createTime`.

### `sha256Hash`

- **Type:** string (bytes format)
- **Description:** Output only. SHA-256 hash of the uploaded bytes, encoded as a base64 string.

### `uri`

- **Type:** string
- **Description:** Output only. The URI of the File.

## Methods

### `delete`

Deletes the specified File.

### `get`

Retrieves metadata for a given File.

### `list`

Lists metadata for Files owned by the requesting project.

# Media Upload Method Documentation

This section details the `media.upload` method used to upload a local file to create a File resource.

## Method: `media.upload`

Uploads a local file to create a File resource.

### Example Usage

For an example of how to use the upload service, refer to the provided Colab notebook.

### HTTP Request

#### Upload URI (for media upload requests)

```
POST https://generativelanguage.googleapis.com/upload/v1beta/files
```

#### Metadata URI (for metadata-only requests)

```
POST https://generativelanguage.googleapis.com/v1beta/files
```

The URL uses gRPC Transcoding syntax.

### Request Body

The request body should contain data with the following structure:

```
{
"file": {
object (File)
}
}
```

#### Fields

- `file`: object (File) - Optional. Metadata for the file to create.

### Response Body

The response for `media.upload` contains data structured as follows:

```
{
"file": {
object (File)
}
}
```

#### Fields

- `file`: object (File) - Metadata for the created file.

# Example File Upload via curl

#!/bin/sh

#

# Upload a file using the GenAI File API via curl.

```bash
api_key=""
input_file=""
display_name=""

while getopts a:i:d: flag
do
case "${flag}" in
a) api_key=${OPTARG};;
i) input_file=${OPTARG};;
d) display_name=${OPTARG};;
esac
done

BASE_URL="https://generativelanguage.googleapis.com"

CHUNK_SIZE=8388608 # 8 MiB
MIME_TYPE=$(file -b --mime-type "${input_file}")
NUM_BYTES=$(wc -c < "${input_file}")

echo "Starting upload of '${input_file}' to ${BASE_URL}..."
echo "  MIME type: '${MIME_TYPE}'"
echo "  Size: ${NUM_BYTES} bytes"

# Initial resumable request defining metadata.

tmp_header_file=$(mktemp /tmp/upload-header.XXX)
curl "${BASE_URL}/upload/v1beta/files?key=${api_key}" \
-D "${tmp_header_file}" \
-H "X-Goog-Upload-Protocol: resumable" \
-H "X-Goog-Upload-Command: start" \
-H "X-Goog-Upload-Header-Content-Length: ${NUM_BYTES}" \
-H "X-Goog-Upload-Header-Content-Type: ${MIME_TYPE}" \
-H "Content-Type: application/json" \
-d "{'file': {'display_name': '${display_name}'}}"
upload_url=$(grep "x-goog-upload-url: " "${tmp_header_file}" | cut -d" " -f2 | tr -d "\r")
rm "${tmp_header_file}"

if [[ -z "${upload_url}" ]]; then
echo "Failed initial resumable upload request."
exit 1
fi

# Upload the actual bytes.

NUM_CHUNKS=$(((NUM_BYTES + CHUNK_SIZE - 1) / CHUNK_SIZE))
tmp_chunk_file=$(mktemp /tmp/upload-chunk.XXX)
for i in $(seq 1 ${NUM_CHUNKS})
do
offset=$((i - 1))
byte_offset=$((offset * CHUNK_SIZE))

# Read the actual bytes to the tmp file.

dd skip="${offset}" bs="${CHUNK_SIZE}" count=1 if="${input_file}" of="${tmp_chunk_file}" 2>/dev/null
num_chunk_bytes=$(wc -c < "${tmp_chunk_file}")
upload_command="upload"
if [[ ${i} -eq ${NUM_CHUNKS} ]] ; then

# For the final chunk, specify "finalize".

upload_command="${upload_command}, finalize"
fi
echo "  Uploading ${byte_offset} - $((byte_offset + num_chunk_bytes)) of ${NUM_BYTES}..."
curl "${upload_url}" \
-H "Content-Length: ${num_chunk_bytes}" \
-H "X-Goog-Upload-Offset: ${byte_offset}" \
-H "X-Goog-Upload-Command: ${upload_command}" \
--data-binary "@${tmp_chunk_file}"
done

rm "${tmp_chunk_file}"

echo "Upload complete!"
```