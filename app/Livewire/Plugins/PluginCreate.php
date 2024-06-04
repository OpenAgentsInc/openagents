<?php

namespace App\Livewire\Plugins;

use App\Models\Plugin;
use App\Rules\WasmFile;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;
use Livewire\WithFileUploads;

class PluginCreate extends Component
{
    use LivewireAlert, WithFileUploads;

    protected $listeners = ['tags-updated' => 'updateTags'];

    // public $kind;

    public $name;

    public $description;

    public $tos;

    public $privacy;

    public $web;

    public $picture;

    public $wasm_upload;

    public $tags = [];

    public $inputs = [];

    public $file_link;

    public $secrets = [];

    public $input_template = '{{in.Input0}}';

    public $payment;
    public $allowed_hosts = [];

    public $user;

    public Plugin $plugin;


    public function mount()
    {

        if (! auth()->check()) {
            return redirect('/');
        }

        $user = auth()->user();
        $this->user = $user;



        $this->inputs[] = [
            'name' => 'Input0',
            'required' => true,
            'type' => 'string',
            'description' => 'An input',
        ];

        // $this->secrets[] = [
        //     'key' => '',
        //     'value' => '',
        // ];
        if (isset($this->plugin)) {
            abort_if($user->id !== $this->plugin->user_id, 403, 'permission denied');
            $this->loadPluginProperties();
        }

    }

    public function loadPluginProperties()
    {
        $this->name = $this->plugin->name;
        $this->description = $this->plugin->description;
        $this->tos = $this->plugin->tos;
        $this->privacy = $this->plugin->privacy;
        $this->payment = $this->plugin->payment ? $this->plugin->payment : $this->plugin->user->lightning_address;
        $this->web = $this->plugin->web;
        $this->picture = $this->plugin->picture;
        $this->tags = $this->plugin->tags;
        $this->inputs = json_decode($this->plugin->input_sockets, true);
        $this->secrets = json_decode($this->plugin->secrets, true);
        $this->input_template = $this->plugin->input_template;
        $this->file_link = $this->plugin->file_link;

        // if(isset($this->plugin->wasm_upload)){
        //     $this->wasm_upload = json_decode($this->plugin->wasm_upload);
        // }

        if(isset($this->plugin->allowed_hosts)){
            $this->allowed_hosts = json_decode($this->plugin->allowed_hosts, true);
        }
        if (isset($this->plugin->tags)) {
            $this->tags = json_decode($this->plugin->tags, true);
        }
        // HOTFIX: if not array reset to empty array
        if (! is_array($this->tags)) {
            $this->tags = '[]';
        }
    }

    public function rules()
    {
        return [
            'name' => 'required|string',
            'description' => 'required|string',
            'tos' => 'required|string',
            'privacy' => 'required|string',
            'payment' => 'nullable|string',
            'web' => 'nullable|string',

            'wasm_upload' => ['nullable', 'file', new WasmFile()],
            'secrets' => 'nullable|array',
            'secrets.*.key' => 'required_with:secrets.*.value|string',
            'secrets.*.value' => 'required_with:secrets.*.key|string',
            'input_template' => 'required|string',
            'inputs' => 'required|array',
            'inputs.*.name' => 'required|string',
            'inputs.*.description' => 'required|string',
            'inputs.*.default' => 'nullable|string',
            'inputs.*.required' => 'required|boolean',
            'inputs.*.type' => 'required|string|in:string,integer,object,array',
            'allowed_hosts' => 'nullable|array',
            'allowed_hosts.*' => 'required|string',

        ];
    }

    public function submit()
    {
        $validated = $this->validate();

        $plugin = null;
        $update = true;
        if (! isset($this->plugin)) {
            $plugin = new Plugin();
            $update = false;
            $plugin->user_id = auth()->user()->id;
        } else {
            abort_if(auth()->user()->id !== $this->plugin->user_id, 403, 'permission denied');
            $plugin = $this->plugin;
        }

        if (! is_null($this->wasm_upload) || ! empty($this->wasm_upload)) {

            if ($update) {
                $oldFile = json_decode($plugin->wasm_upload);
                if ($oldFile && isset($oldFile->path)) {
                    Storage::disk($oldFile->disk)->delete($oldFile->path);
                }
            }

            $disk = config('filesystems.media_disk');

            // Get filename with extension
            $filenameWithExt = $this->wasm_upload->getClientOriginalName();

            // Get filename without extension
            $filename = pathinfo($filenameWithExt, PATHINFO_FILENAME);

            // Get file extension
            $extension = $this->wasm_upload->getClientOriginalExtension();

            // Filename to store with directory
            $path = 'wasm/uploads/'.str($filename)->slug()->toString().'_'.time().'.'.$extension;

            // Upload File to public
            Storage::disk($disk)->put($path, fopen($this->wasm_upload->getRealPath(), 'r+'), 'public');
            $url = Storage::disk($disk)->url($path);

            $plugin->wasm_upload = json_encode([
                'disk' => $disk,
                'path' => $path,
                'url' => $url,
            ]);
            $this->file_link = $url;
        } elseif (! $update) {
            $this->alert('error', 'Wasm file is required');

            return;
        }

        try {
            $plugin->name = $this->name;
            $plugin->description = $this->description;
            $plugin->tos = $this->tos;
            $plugin->privacy = $this->privacy;
            $plugin->web = $this->web;
            $plugin->picture = $this->picture;
            $plugin->tags = json_encode($this->tags);
            $plugin->output_sockets = json_encode([
                'output' => [
                    'title' => 'Output',
                    'description' => 'The output',
                    'type' => 'string',
                ],
            ]);
            $plugin->input_sockets = json_encode($this->inputs);
            $plugin->input_template = $this->input_template;

            $plugin->secrets = isset($this->secrets) ? json_encode($this->secrets) : '[]';
            $plugin->file_link = $this->file_link;

            $plugin->payment = $this->payment;
            $plugin->allowed_hosts = isset($this->allowed_hosts) ? json_encode($this->allowed_hosts) : '[]';
            $plugin->tags = isset($this->tags) ? json_encode($this->tags) : '[]';

            $plugin->save();
            $good = true;
        } catch (\Throwable $th) {
            Log::error('error forom plugin : '.$th);
            $good = false;
        }

        if ($good) {
            $this->alert('success', 'Plugin successfully created.');

            return redirect()->route('plugins.index');
        } else {
            $this->alert('error', 'An error occured');
        }
    }

    public function addInput()
    {
        $this->inputs[] = [
            'name' => '',
            'required' => false,
            'type' => 'string',
            'description' => '',
            'default' => '',
        ];
    }

    public function removeInput($key)
    {
        unset($this->inputs[$key]);
        $this->inputs = array_values($this->inputs);
    }

    public function addAllowedHost()
    {
        $this->allowed_hosts[] = '';
    }

    public function removeAllowedHost($key)
    {
        unset($this->allowed_hosts[$key]);
        $this->allowed_hosts = array_values($this->allowed_hosts);
    }

    public function addSecretInput()
    {
        $this->secrets[] = [
            'key' => '',
            'value' => '',
        ];
    }

    public function removeSecretInput($key)
    {
        unset($this->secrets[$key]);
        $this->secrets = array_values($this->secrets);

    }

    public function render()
    {
        return view('livewire.plugins.plugin-create');
    }

    public function updateTags($tags)
    {
        $this->tags = $tags;
    }
}
