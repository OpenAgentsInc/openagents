<?php

namespace App\Http\Controllers;

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
          'file' => 'required|mimetypes:application/json,application/pdf,text/markdown,text/plain', // |max:1000240
        ]);
      }

      // Retrieve the uploaded file
      $file = $request->file('file');

      // Store the file
      $path = Storage::putFile('uploads', $file);

      // Logic to ingest the file content, create finetuning job, etc.

      return Redirect::route('start')->with('message', 'File uploaded.');
    } catch (\Exception $e) {
      dd($e);
      return Redirect::route('start')->with('error', 'Error uploading file.');
    }
  }
}
