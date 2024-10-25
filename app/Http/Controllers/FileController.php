<?php

namespace App\Http\Controllers;

use App\Models\File;
use App\Models\Project;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Facades\Log;
use Spatie\PdfToText\Pdf;

class FileController extends Controller
{
    public function store(Request $request)
    {
        try {
            Log::info('File upload started', ['request' => $request->all()]);

            // Validate incoming request
            $request->validate([
                'file' => 'required|mimetypes:application/json,application/pdf,text/markdown,text/plain,image/jpeg,image/png,image/gif',
            ]);

            // Handle project_id (null or provided)
            $projectId = $request->input('project_id');
            if (is_null($projectId)) {
                $project = Project::create([
                    'name' => 'Dummy Project ' . now()->format('Y-m-d H:i:s'),
                    'description' => 'This is a dummy project created for file upload.',
                ]);
                $projectId = $project->id;
                Log::info('Dummy project created', ['project_id' => $projectId, 'project_name' => $project->name]);
            } else {
                Log::info('Using existing project', ['project_id' => $projectId]);
            }

            // Store the file
            $path = $request->file('file')->store('uploads');
            Log::info('File stored', ['path' => $path]);

            // Extract content based on file type
            $content = $this->extractContent($path, $request->file('file')->getMimeType());
            Log::info('Content extracted', ['content_length' => strlen($content)]);

            // Create a new File record
            $file = File::create([
                'name' => $request->file('file')->getClientOriginalName(),
                'path' => $path,
                'content' => $content,
                'project_id' => $projectId,
            ]);
            Log::info('File record created', ['file_id' => $file->id, 'project_id' => $projectId]);

            return redirect()->route('home')
                ->with('message', 'File uploaded and ingested.')
                ->with('filename', $file->name);
        } catch (\Exception $e) {
            Log::error('Error uploading file', [
                'error' => $e->getMessage(),
                'trace' => $e->getTraceAsString(),
            ]);
            return redirect()->route('home')->with('error', 'Error uploading file: ' . $e->getMessage());
        }
    }

    private function extractContent($path, $mimeType)
    {
        if (!Storage::exists($path)) {
            throw new \Exception("File not found: {$path}");
        }

        switch ($mimeType) {
            case 'application/pdf':
                $pdfToTextPath = config('services.pdftotext.path');
                if (!$pdfToTextPath || !file_exists($pdfToTextPath)) {
                    throw new \Exception("PDF to Text converter not found. Please check the configuration.");
                }
                $text = Pdf::getText(Storage::path($path), $pdfToTextPath);
                if (empty($text)) {
                    throw new \Exception("Failed to extract text from PDF: {$path}");
                }
                return $text;
            case 'text/plain':
            case 'text/markdown':
            case 'application/json':
                return Storage::get($path);
            case 'image/jpeg':
            case 'image/png':
            case 'image/gif':
                // For images, we'll store the file path as content
                return $path;
            default:
                throw new \Exception('Unsupported file type: ' . $mimeType);
        }
    }
}