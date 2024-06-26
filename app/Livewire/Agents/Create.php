<?php

namespace App\Livewire\Agents;

use App\AI\Models;
use App\Models\Agent;
use App\Utils\PoolUtils;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Attributes\Computed;
use Livewire\Component;
use Livewire\WithFileUploads;

class Create extends Component
{
    use LivewireAlert, WithFileUploads;

    public $name;

    public $about;

    public $codebase_search;

    public $prompt;

    public $model;

    public $pro_model;

    public $models = Models::MODELS;

    public $free_agent_models = [];

    public $all_agent_models = [];

    public $rag_prompt;

    public $is_public = true;

    public $files = [];

    public $urls = '';

    public $image;

    public $message;

    public $plugins = [];

    public function mount()
    {
        if (! auth()->check()) {
            return redirect('/');
        }
        $this->free_agent_models = Models::getSelectModelsForUserTypes(['guest', 'user']);
        $this->all_agent_models = Models::getSelectModelsForUserTypes(['guest', 'user', 'pro']);
    }

    public function rules()
    {
        return [
            'name' => 'required|string|max:255',
            'about' => 'required|string',
            'prompt' => 'required|string',
            'model' => 'nullable|in:'.implode(',', Models::getModelsForUserTypes(['guest', 'user'])),
            'pro_model' => 'nullable|in:'.implode(',', Models::getModelsForUserTypes(['guest', 'user', 'pro'])),
            //            'rag_prompt' => 'nullable|string',
            'message' => 'required|string',
            'is_public' => 'required|boolean',
            'files' => 'nullable|array',
            'files.*' => 'nullable|file|mimes:txt,pdf,xls,doc,docx,xlsx,csv|max:10240',
            'image' => 'nullable|image|max:2048',
            'urls' => 'nullable|string',
        ];
    }

    public function submit()
    {
        //    $this->validate();

        $user = auth()->user();
        if (! $user) {
            Log::error('User not found');

            return redirect('/');
        }

        $agent = new Agent();

        // Upload file

        if (! is_null($this->image) || ! empty($this->image)) {

            $disk = config('filesystems.media_disk');

            // Get filename with extension
            $filenamewithextension = $this->image->getClientOriginalName();

            // Get filename without extension
            $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

            // Get file extension
            $extension = $this->image->getClientOriginalExtension();

            // Filename to store with directory
            $filenametostore = 'agents/profile/images/'.str($filename)->slug()->toString().'_'.time().'.'.$extension;

            // Upload File to public
            Storage::disk($disk)->put($filenametostore, fopen($this->image->getRealPath(), 'r+'), 'public');

            $saveimage = [
                'disk' => $disk,
                'path' => $filenametostore,
                'url' => Storage::disk($disk)->url($filenametostore),
            ];
        } else {
            $saveimage = [
                'disk' => null,
                'path' => null,
                'url' => url('/images/sqlogo.png'),
            ];
        }

        $agent->name = $this->name;
        $agent->about = $this->about;
        $agent->prompt = $this->prompt;
        $agent->model = $this->model;
        $agent->pro_model = $this->pro_model;

        if ($this->codebase_search) {
            $agent->capabilities = json_encode(['codebase_search' => true]);
        }

        $agent->rag_prompt = 'placeholder'; // $this->rag_prompt;
        $agent->is_public = $this->is_public;
        $agent->message = $this->message;
        $agent->image = json_encode($saveimage);
        $agent->user_id = $user->id;
        $agent->is_rag_ready = empty($this->files);
        $agent->save();

        $needWarmUp = false;
        if (! empty($this->files)) {
            $needWarmUp = true;
            $disk = config('documents.disk');
            foreach ($this->files as $file) {
                // Get filename with extension
                $filenamewithextension = $file->getClientOriginalName();

                // Get filename without extension
                $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

                // Get file extension
                $extension = $file->getClientOriginalExtension();

                // Filename to store with directory
                $filenametostore = 'agents/files/'.str($filename)->slug()->toString().'_'.time().'.'.$extension;

                // Upload File to public
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
        }

        if (! empty($this->urls)) {
            $needWarmUp = true;
            $urls = explode("\n", trim($this->urls));
            foreach ($urls as $url) {
                $url = trim($url);
                if (! empty($url) && filter_var($url, FILTER_VALIDATE_URL)) {
                    $agent->documents()->create([
                        'name' => $url,
                        'path' => $url,
                        'url' => $url,
                        'disk' => 'url',
                        'type' => 'url',
                    ]);
                }
            }
        }

        $agent->save();

        foreach ($this->plugins as $plugin) {
            $agent->externalTools()->create([
                'external_uid' => $plugin,
            ]);
        }

        if ($needWarmUp) {
            // Log::info('Agent created with documents', ['agent' => $agent->id, 'documents' => $agent->documents()->pluck('url')->toArray()]);
            // Send RAG warmup request
            PoolUtils::sendRAGWarmUp($agent->id, -1, 'agentbuilder'.PoolUtils::uuid(), $agent->documents()->pluck('url')->toArray());
            $this->alert('success', 'Agent training process has now begin ..');
        } else {
            $this->alert('success', 'Agent created successfully..');
        }

        session()->put('agent', $agent->id);

        return redirect("/chat?agent={$agent->id}");
    }

    #[Computed]
    public function list_plugins()
    {

        $tools = PoolUtils::getTools();
        $out = [];
        foreach ($tools as $tool) {
            $out[$tool['id']] = $tool['meta']['name'];
        }

        return $out;
    }

    public function render()
    {
        return view('livewire.agents.create');
    }
}
