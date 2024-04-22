<?php

namespace App\Livewire\Agents;

use App\Models\Agent;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;
use Livewire\WithFileUploads;

class Create extends Component
{
    use LivewireAlert, WithFileUploads;

    public $name;

    public $about;

    public $prompt;

    public $rag_prompt;

    public $is_public = true;

    public $files = [];

    public $image;

    public $message;

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

        $this->validate();

        $user = auth()->user();

        $agent = new Agent();

        // Upload file

        if (! is_null($this->image) || ! empty($this->image)) {

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
                'disk' => 'public',
                'path' => $filenametostore,
                'url' => Storage::disk('s3')->url($filenametostore),
            ];
        } else {
            $saveimage = [
                'disk' => null,
                'path' => null,
                'url' => url('/images/no-image.jpg'),
            ];
        }

        $agent->name = $this->name;
        $agent->about = $this->about;
        $agent->prompt = $this->prompt;
        $agent->rag_prompt = $this->rag_prompt;
        $agent->is_public = $this->is_public;
        $agent->message = $this->message;
        $agent->image = json_encode($saveimage);
        $agent->user_id = $user->id;
        $agent->save();

        if (! empty($this->files)) {
            foreach ($this->files as $file) {
                // Get filename with extension
                $filenamewithextension = $file->getClientOriginalName();

                // Get filename without extension
                $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

                // Get file extension
                $extension = $file->getClientOriginalExtension();

                // Filename to store with directory
                $filenametostore = 'agents/files/documents/'.$filename.'_'.time().'.'.$extension;

                // Upload File to public
                Storage::disk('s3')->put($filenametostore, fopen($file->getRealPath(), 'r+'), 'public');

                $url = Storage::disk('s3')->url($filenametostore);

                $agent->documents()->create([
                    'name' => $filename,
                    'path' => $filenametostore,
                    'url' => $url,
                    'disk' => 's3',
                    'type' => $file->getClientMimeType(),
                ]);
            }
        }

        // session()->flash('message', 'Form submitted successfully!');
        $this->alert('success', 'Form submitted successfully');

        $this->reset(); // Reset form after successful submission

        return redirect()->route('agents');
    }

    public function render()
    {
        return view('livewire.agents.create');
    }
}
