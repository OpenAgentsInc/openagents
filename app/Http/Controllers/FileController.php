<?php

namespace App\Http\Controllers;

use App\Jobs\IngestPDF;
use App\Models\Agent;
use App\Models\Brain;
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
            'file' => 'required|mimetypes:application/pdf',
            'agent_id' => 'required',
        ]);

        try {
            // Store the file to Laravel storage
            $thefile = $request->file('file');
            // Get the file's name
            $filename = $thefile->getClientOriginalName();

            $path = Storage::putFile('uploads', $thefile);

            // Create a new file record
            $file = File::create([
                'user_id' => auth()->user()->id,
                'agent_id' => request('agent_id'),
                // 'conversation_id' => request('conversation_id'),
                'name' => $thefile->getClientOriginalName(),
                'path' => $path,
                'size' => $thefile->getSize(),
                'mime_type' => $thefile->getMimeType(),
                'status' => 'processing'
            ]);

            // If we have a brain, parse the file
            $agent = Agent::findOrFail(request('agent_id'));
            $brain = $this->getBrain();

            // Fire new IngestPDF job
            IngestPDF::dispatch($path, $agent, $brain, $file);

            return Redirect::back()
                ->with('message', 'File uploaded.')
                ->with('filename', $file->name);

        } catch (\Exception $e) {
            // Log just the error message
            Log::error('Error uploading file' . print_r($e->getMessage(), true));
            return Redirect::back()->with('error', 'Error uploading file.');
        }
    }

    private function getBrain()
    {
        $brain = null;
        if (request('agent_id')) {
            $agent = Agent::findOrFail(request('agent_id'));

            if ($agent->brains->count() > 0) {
                $brain = $agent->brains->first();
            } else {
                // Create a new brain for this agent
                $brain = Brain::create([
                    'agent_id' => $agent->id,
                ]);
            }
        } else {
            return $brain;
        }
        return $brain;
    }
}
