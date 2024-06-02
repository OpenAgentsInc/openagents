<?php

namespace App\Livewire\Plugins;

use App\Models\Plugin;
use App\Rules\WasmFile;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;
use Livewire\WithFileUploads;

class PluginCreate extends Component
{
    use LivewireAlert, WithFileUploads;

    // public $kind;

    public $name;

    public $description;

    public $tos;

    public $privacy;

    public $author;

    public $web;

    public $picture;

    public $wasm_upload;

    public $tags;

    public $inputs=[];

    public $file_link;

    public $secrets=[];

    public $input_template="{{in.input0}}";

    public $payment;

    public $user;

    public Plugin $plugin;


    public function mount()
    {


        if (! auth()->check()) {
            return redirect('/');
        }

        $user = auth()->user();
        $this->user = $user;

        if(isset($this->plugin)){
            abort_if($user->id !== $this->plugin->user_id, 403, 'permission denied');
            $this->loadPluginProperties();
        }

        $this->inputs[]= [
            'name' => 'Input0',
            'required' => true,
            'type' => 'string',
            'description' => 'An input',
        ];

        $this->secrets[]= [
            'key' => '',
            'value' => '',
        ];
    }

    public function loadPluginProperties()
    {
        $this->name = $this->plugin->name;
        $this->description = $this->plugin->description;
        $this->tos = $this->plugin->tos;
        $this->privacy = $this->plugin->privacy;
        $this->author = $this->plugin->author ? $this->author : $this->plugin->user->name;
        $this->payment = $this->plugin->payment ? $this->plugin->payment : $this->plugin->user->lightning_address;
        $this->web = $this->plugin->web;
        $this->picture = $this->plugin->picture;
        $this->tags = $this->plugin->tags;
        $this->inputs = json_decode($this->plugin->input_sockets, true);
        $this->secrets = json_decode($this->plugin->secrets, true);
        $this->input_template = $this->plugin->input_template;
        $this->file_link = $this->plugin->file_link;
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
            // 'picture' => 'nullable|string',
            // 'tags' => 'required|array',
            // 'file_link' => ['required', 'string', 'url', 'active_url', new WasmUrl()],
            'wasm_upload' => ['required', 'file', new WasmFile()],
            'secrets' => 'nullable|array',
            'secrets.*.key' => 'required_with:secrets.*.value|string',
            'secrets.*.value' => 'required_with:secrets.*.key|string',
            'input_template' => 'required|string',
            'inputs' => 'required|array',
            'inputs.*.name' => 'required|string',
            'inputs.*.description' => 'required|string',
            'inputs.*.default' => 'required|string',
            'inputs.*.required' => 'required|boolean',
            'inputs.*.type' => 'required|string|in:string,integer,object,array',

        ];
    }

    public function submit()
    {
        $validated = $this->validate();

        $plugin = $this->plugin;
        $update = true;
        if(!isset($plugin)){
            $plugin = new Plugin();
            $update = false;
        }

        if (! is_null($this->wasm_upload) || ! empty($this->wasm_upload)) {

            if($update){
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
            // $savewasm_upload = [
            //     'disk' => $disk,
            //     'path' => $path,
            //     'url' => $url,
            // ];
            // $wasm_upload = collect($savewasm_upload);
            $this->file_link = $url;
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
            $plugin->input_sockets = $this->inputs->toJson();
            $plugin->input_template = $this->input_template;

            $plugin->secrets = $this->secrets->toJson();
            $plugin->file_link = $this->file_link;
            $plugin->user_id = auth()->user()->id;
            $plugin->author = auth()->user()->name;
            $plugin->payment = $this->payment;
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
        $this->inputs[]=[
            'name' => '',
            'required' => false,
            'type' => 'string',
            'description' => '',
            'default' => ''
        ];
    }

    public function removeInput($key)
    {
        unset($this->inputs[$key]);
        $this->inputs = array_values($this->inputs);
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


}
