# File Uploading and Project Association

This document outlines the file uploading functionality and how files are associated with projects in our application.

## Overview

Our application allows users to upload files and associate them with specific projects. The system supports various file types, including PDFs, and automatically extracts and stores the content of these files for further processing.

## File Model

The `File` model represents uploaded files in the database. It has the following key attributes:

- `name`: The original name of the uploaded file
- `path`: The storage path of the file in the system
- `content`: The extracted text content of the file (for supported file types)
- `project_id`: The ID of the project associated with this file

## File Upload Process

1. The user selects a file to upload through the `UploadDocForm` component.
2. The component sends a POST request to `/api/files` with the file and the associated `project_id`.
3. The `FileController` handles the upload request:
   - Validates the incoming file
   - Stores the file in the `uploads` directory
   - Extracts the content from the file (if it's a supported type)
   - Creates a new `File` record in the database with the file details and extracted content
   - Associates the file with the specified project

## Supported File Types

The application currently supports the following file types:

- PDF (.pdf)
- Plain Text (.txt)
- Markdown (.md)
- JSON (.json)

For PDF files, the system uses the Spatie PDF to Text package to extract the content.

## UploadDocForm Component

The `UploadDocForm` React component provides the user interface for file uploads. It accepts a `projectId` prop to associate the uploaded file with the correct project.

Usage example:

```jsx
<UploadDocForm projectId={currentProject.id} />
```

## File Controller

The `FileController` handles the server-side logic for file uploads. Key methods:

- `store(Request $request)`: Handles the file upload, content extraction, and database storage.

## Database Schema

The `files` table in the database has the following structure:

```
id: bigint(20) unsigned AUTO_INCREMENT PRIMARY KEY
name: varchar(255)
path: varchar(255)
content: text NULL
project_id: bigint(20) unsigned
created_at: timestamp NULL
updated_at: timestamp NULL
```

## Testing

The `ProjectTest` includes tests to verify the file upload functionality and the association between files and projects. Key test:

- `test('a file can be uploaded and associated with a project')`

## Error Handling

The file upload process includes error handling for:
- Invalid file types
- File size limits
- Database errors
- Content extraction failures

Errors are returned to the frontend and displayed to the user through the `UploadDocForm` component.

## Security Considerations

- File uploads are restricted to authenticated users
- File types are validated to prevent malicious file uploads
- File size limits are enforced to prevent server overload
- Uploaded files are stored outside the web root to prevent direct access

## Future Improvements

- Support for additional file types (e.g., docx, xlsx)
- Implement file versioning
- Add file preview functionality
- Implement file sharing between projects or users

For any questions or issues related to file uploading, please contact the development team.