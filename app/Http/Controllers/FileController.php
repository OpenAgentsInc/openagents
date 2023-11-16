<?php

namespace App\Http\Controllers;

use App\Services\Vectara;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Redirect;
use Illuminate\Support\Facades\Storage;

class FileController extends Controller
{
    public function store(Request $request)
    {
        try {
            // If we're not testing
            if (!app()->runningUnitTests()) {
                // Validate incoming request
                $request->validate([
                  'file' => 'required|mimetypes:application/pdf'
                  // 'file' => 'required|mimetypes:application/json,application/pdf,text/markdown,text/plain', // |max:1000240
                ]);
            }

            // Store the file
            $file = $request->file('file');
            $path = Storage::putFile('uploads', $file);

            // Parse the file

            // $file = new \Illuminate\Http\UploadedFile(
            //     storage_path('app/' . $path),
            //     $file->getClientOriginalName(),
            //     $file->getMimeType(),
            //     null,
            //     true
            // );

            return Redirect::route('start')
              ->with('message', 'File uploaded.')
              ->with('filename', $file->getClientOriginalName());
        } catch (\Exception $e) {
            return Redirect::route('start')->with('error', 'Error uploading file.');
        }
    }
}
