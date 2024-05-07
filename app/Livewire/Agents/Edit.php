<?php

namespace App\Livewire\Agents;

use App\Jobs\ProcessAgentRag;
use App\Models\Agent;
use Livewire\Component;
use App\Jobs\ProcessAgentRag;
use Livewire\WithFileUploads;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;

class Edit extends Component
{
    use LivewireAlert, WithFileUploads;

    public $name;

    public $about;

    public $prompt;

    public $rag_prompt;

    public $is_public;

    public $files = [];

    public $image;

    public $message;

    public Agent $agent;

    public function mount()
    {

        $user = auth()->user();

        abort_if($user->id !== $this->agent->user_id, 403, 'permission denied').

    $this->name = $this->agent->name;
        $this->about = $this->agent->about;
        $this->prompt = $this->agent->prompt;
        $this->rag_prompt = $this->agent->rag_prompt;
        $this->is_public = $this->agent->is_public;
        $this->message = $this->agent->message;

        // public $files = [];
        // public $image;

    }

    public function rules()
    {
        return [
            'name' => 'required|string|max:255',
            'about' => 'required|string',
            'prompt' => 'required|string',
            'rag_prompt' => 'nullable|string',
            'message' => 'required|string',
            'is_public' => 'required|boolean',
            'files' => 'nullable|array',
            'files.*' => 'nullable|file|mimes:txt,pdf,xls,doc,docx,xlsx,csv|max:10240',
            'image' => 'nullable|image|max:2048',
        ];
    }

    public function submit()
    {

        $user = auth()->user();

        $agent = $this->agent;

        $this->validate();

        $saveimage = [];

        // Upload file

        if ($this->image && ! empty($this->image)) {

            $oldimage = json_decode($this->agent->image);

            if ($oldimage && isset($oldimage->path)) {
                $path = $oldimage->path;
                $disk = $oldimage->disk;
                Storage::disk($disk)->delete($path);
            }

            $disk = config('filesystems.default');

            // Get filename with extension
            $filenamewithextension = $this->image->getClientOriginalName();

            // Get filename without extension
            $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

            // Get file extension
            $extension = $this->image->getClientOriginalExtension();

            // Filename to store with directory
            $filenametostore = 'agents/profile/images/'.$filename.'_'.time().'.'.$extension;

            // Upload File to public
            Storage::disk('s3')->put($filenametostore, fopen($this->image->getRealPath(), 'r+'), 'public');

            $saveimage = [
                'disk' => 's3',
                'path' => $filenametostore,
                'url' => Storage::disk('s3')->url($filenametostore),
            ];
        }

        $agent->name = $this->name;
        $agent->about = $this->about;
        $agent->prompt = $this->prompt;
        $agent->rag_prompt = $this->rag_prompt;
        $agent->is_public = $this->is_public;
        $agent->message = $this->message;
        if ($this->image) {
            $agent->image = json_encode($saveimage);
        }

        $agent->user_id = $user->id;
        $agent->save();

        if (! empty($this->files)) {
            $disk = config('documents.disk');

            foreach ($this->files as $file) {
                // Get filename with extension
                $filenamewithextension = $file->getClientOriginalName();

                // Get filename without extension
                $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

                // Get file extension
                $extension = $file->getClientOriginalExtension();

                // Filename to store with directory
                $filenametostore = 'agents/files/documents/'.$filename.'_'.time().'.'.$extension;

                // Upload File to s3
                Storage::disk($disk)->put($filenametostore, fopen($file->getRealPath(), 'r+'), 'public');

                $url = Storage::disk($disk)->url($filenametostore);

                $agent->documents()->create([
                    'name' => $filename,
                    'path' => $filenametostore,
                    'url' => $url,
                    'disk' => $disk,
                    'type' => $file->getClientMimeType(),
                ]);
            }

            // Send documents to Nostr for RAG

            ProcessAgentRag::dispatch($agent);
        }

        $this->alert('success', 'Agent updated successfully');

    }

    public function render()
    {
        return view('livewire.agents.edit');
    }
}
