<?php

namespace App\Http\Controllers;

use App\Jobs\IngestPDF;
use App\Models\File;
use App\Services\Parser;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Redirect;
use Illuminate\Support\Facades\Storage;

class FileController extends Controller
{
    public function store(Request $request)
    {
        // Validate the file is a PDF
        $request->validate([
            'file' => 'required|mimetypes:application/pdf'
        ]);

        try {
            // Store the file to Laravel storage
            $thefile = $request->file('file');
            $path = Storage::putFile('uploads', $thefile);

            // Create a new file record
            $file = File::create([
                'user_id' => auth()->user()->id,
                'path' => $path,
                'status' => 'processing'
            ]);

            // Fire new IngestPDF job
            IngestPDF::dispatch($file);

            return Redirect::back()
                ->with('message', 'File uploaded.')
                ->with('filename', $res["file_id"]);

        } catch (\Exception $e) {
            // Log just the error message
            Log::error('Error uploading file' . print_r($e->getMessage(), true));
            return Redirect::back()->with('error', 'Error uploading file.');
        }
    }
}
