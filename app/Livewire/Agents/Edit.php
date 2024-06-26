<?php

namespace App\Livewire\Agents;

use App\AI\Models;
use App\Livewire\Agents\Partials\Documents;
use App\Models\Agent;
use App\Models\AgentFile;
use App\Utils\PoolUtils;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Attributes\Computed;
use Livewire\Component;
use Livewire\WithFileUploads;

class Edit extends Component
{
    use LivewireAlert, WithFileUploads;

    public $name;

    public $about;

    public $prompt;

    public $model;

    public $pro_model;

    public $models = Models::MODELS;

    public $free_agent_models = [];

    public $all_agent_models = [];

    public $rag_prompt;

    public $is_public;

    public $files = [];

    public $urls = '';

    public $image;

    public $message;

    public $sats_per_message;

    public Agent $agent;

    public $plugins = [];

    public $ownerId;

    public function mount()
    {

        if (! auth()->check()) {
            return redirect('/');
        }

        $user = auth()->user();
        abort_if($user->id != $this->agent->user_id && ! $user->getRole()->canModerate($this->agent->user->getRole()), 403, 'permission denied');

        $this->name = $this->agent->name;
        $this->about = $this->agent->about;
        $this->prompt = $this->agent->prompt;
        $this->model = $this->agent->model;
        $this->pro_model = $this->agent->pro_model;
        $this->rag_prompt = $this->agent->rag_prompt;
        $this->is_public = $this->agent->is_public;
        $this->sats_per_message = $this->agent->sats_per_message;
        $this->message = $this->agent->message;
        $this->plugins = $this->agent->externalTools()->pluck('external_uid');

        $this->free_agent_models = Models::getSelectModelsForUserTypes(['guest', 'user']);
        $this->all_agent_models = Models::getSelectModelsForUserTypes(['guest', 'user', 'pro']);
        $this->ownerId = $this->agent->user_id;

        $docs = AgentFile::with('agent')
            ->where('agent_id', $this->agent->id)
            ->get();
        $docs = $docs->filter(function ($doc) {
            return $doc->type === 'url';
        });
        $docs = $docs->pluck('url')->implode("\n");
        $this->urls = $docs;
    }

    public function rules()
    {
        return [
            'name' => 'required|string|max:255',
            'about' => 'required|string',
            'prompt' => 'required|string',
            'model' => 'nullable|in:'.implode(',', Models::getModelsForUserTypes(['guest', 'user'])),
            'pro_model' => 'nullable|in:'.implode(',', Models::getModelsForUserTypes(['guest', 'user', 'pro'])),
            'rag_prompt' => 'nullable|string',
            //            'message' => 'required|string',
            'is_public' => 'required|boolean',
            'files' => 'nullable|array',
            'files.*' => 'nullable|file|mimes:txt,pdf,xls,doc,docx,xlsx,csv|max:10240',
            'image' => 'nullable|image|max:2048',
            'urls' => 'nullable|string',
            'sats_per_message' => 'required|integer|min:3|max:3000',
        ];
    }

    public function submit()
    {

        $user = auth()->user();
        abort_if($user->id != $this->agent->user_id && ! $user->getRole()->canModerate($this->agent->user->getRole()), 403, 'permission denied');

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

            $disk = config('filesystems.media_disk');

            // Get filename with extension
            $filenamewithextension = $this->image->getClientOriginalName();

            // Get filename without extension
            $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

            // Get file extension
            $extension = $this->image->getClientOriginalExtension();

            // Filename to store with directory
            $filenametostore = '/agents/images/'.str($filename)->slug()->toString().'_'.time().'.'.$extension;

            // Upload File to public
            Storage::disk($disk)->put($filenametostore, fopen($this->image->getRealPath(), 'r+'), 'public');

            $url = Storage::disk($disk)->url($filenametostore);

            $saveimage = [
                'disk' => $disk,
                'path' => $filenametostore,
                'url' => $url,
            ];
        }

        $agent->name = $this->name;
        $agent->about = $this->about;
        $agent->prompt = $this->prompt;
        $agent->model = $this->model;
        $agent->pro_model = $this->pro_model;
        $agent->rag_prompt = $this->rag_prompt;
        $agent->is_public = $this->is_public;
        $agent->message = $this->message;
        $agent->sats_per_message = $this->sats_per_message;
        if ($this->image) {
            $agent->image = json_encode($saveimage);
        }

        $agent->is_rag_ready = empty($this->files);
        if ($user->isModerator()) {
            $agent->user_id = $this->ownerId;
        }
        $agent->save();

        $needWarmUp = false;
        if (! empty($this->files)) {
            $needWarmUp = true;
            $disk = config('documents.disk'); // Assuming you are using the 'public' disk

            foreach ($this->files as $file) {
                // Get filename with extension
                $filenamewithextension = $file->getClientOriginalName();

                // Get filename without extension
                $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

                // Get file extension
                $extension = $file->getClientOriginalExtension();

                // Filename to store with directory
                $filenametostore = 'agents/files/'.$filename.'_'.time().'.'.$extension;

                // Upload File to public disk
                Storage::disk($disk)->put($filenametostore, fopen($file->getRealPath(), 'r+'), 'public');

                // Generate URL without the "storage" part
                $url = Storage::disk($disk)->url($filenametostore);

                $agent->documents()->create([
                    'name' => $filename,
                    'path' => $filenametostore,
                    'url' => $url,
                    'disk' => $disk,
                    'type' => $file->getClientMimeType(),
                ]);
            }

            $this->files = [];

            $this->dispatch('document_updated')->to(Documents::class);

            $this->alert('warning', 'Agent learning process has now begin ..');

        }

        $agent->documents()->where('type', 'url')->delete();
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

        $agent->externalTools()->delete();
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

        $this->alert('success', 'Agent updated successfully');

    }

    #[Computed]
    public function list_plugins()
    {
        $currentToolsIds = $this->agent->externalTools()->pluck('external_uid')->toArray();

        $tools = PoolUtils::getTools();
        $out = [];

        // maintain offline tools
        foreach ($currentToolsIds as $id) {
            $out[$id] = $id;
        }

        foreach ($tools as $tool) {
            $out[$tool['id']] = $tool['meta']['name'];
        }

        return $out;
    }

    public function render()
    {
        return view('livewire.agents.edit');
    }
}
