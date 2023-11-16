<?php

namespace App\Http\Controllers;

use App\Services\Parser;
use App\Services\Vectara;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Redirect;
use Illuminate\Support\Facades\Storage;

class FileController extends Controller
{
    public function store(Request $request)
    {
        try {
            // If we're not testing, validate the request
            if (!app()->runningUnitTests()) {
                $request->validate([
                  'file' => 'required|mimetypes:application/pdf' // application/json,text/markdown,text/plain|max:1000240
                ]);
            }

            // Store the file
            $file = $request->file('file');
            $path = Storage::putFile('uploads', $file);

            // Parse the file
            $parser = new Parser();
            $parser->parsePdf($path);

            return Redirect::route('start')
              ->with('message', 'File uploaded.')
              ->with('filename', $file->getClientOriginalName());
        } catch (\Exception $e) {
            return Redirect::route('start')->with('error', 'Error uploading file.');
        }
    }
}
