<?php

namespace App\Livewire\Plugins;

use App\Models\Plugin;
use App\Models\User;
use App\Rules\WasmFile;
use App\Rules\WasmUrl;
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

    public $mini_template;

    // public $sockets;

    public $input;

    public Collection $inputs;


    public $file_link;

    public $output_description;

    public $output_type;

    public Collection $secrets;

    public $plugin_input;

    public $payment;

    public function mount()
    {

        if (! auth()->check()) {
            return redirect('/');
        }



        $this->fill([
            'inputs' => collect([[
                'name' => '',
                'required' => true,
                'type' => 'string',
                'description' => '',
            ]]),
            'secrets' => collect([[
                'key' => '',
                'value' => '',
            ]]),
        ]);

    }

    public function rules()
    {
        return [
            // 'kind' => 'required|string',
            'name' => 'required|string',
            'description' => 'required|string',
            'tos' => 'required|string',
            'privacy' => 'required|string',
            'payment' => 'nullable|string',
            'web' => 'nullable|string',
            // 'picture' => 'nullable|string',
            // 'tags' => 'required|array',
            // 'mini_template' => 'required|array',
            // 'file_link' => ['required', 'string', 'url', 'active_url', new WasmUrl()],
            'wasm_upload' => ['required', 'file', new WasmFile()],
            'secrets' => 'nullable|array',
            'secrets.*.key' => 'required_with:secrets.*.value|string',
            'secrets.*.value' => 'required_with:secrets.*.key|string',
            'plugin_input' => 'required|string',
            'inputs' => 'required|array',
            'inputs.*.name' => 'required|string',
            'inputs.*.description' => 'required|string',
            'inputs.*.required' => 'required|boolean',
            'inputs.*.type' => 'required|string|in:string,integer,json,array',


        ];
    }

    public function submit()
    {

        // dd($this->inputs);

        $validated = $this->validate();
        $good = false;

        if (! is_null($this->wasm_upload) || ! empty($this->wasm_upload)) {

            $disk = config('filesystems.media_disk');

            // Get filename with extension
            $filenamewithextension = $this->wasm_upload->getClientOriginalName();

            // Get filename without extension
            $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

            // Get file extension
            $extension = $this->wasm_upload->getClientOriginalExtension();

            // Filename to store with directory
            $filenametostore = 'wasm/uploads/'.str($filename)->slug()->toString().'_'.time().'.'.$extension;

            // Upload File to public
            Storage::disk($disk)->put($filenametostore, fopen($this->wasm_upload->getRealPath(), 'r+'), 'public');
            $url = Storage::disk($disk)->url($filenametostore);
            $savewasm_upload = [
                'disk' => $disk,
                'path' => $filenametostore,
                'url' => $url,
            ];
            $wasm_upload = collect($savewasm_upload);
            $this->file_link = $url;
        }

        try {

            $plugin = new Plugin();
            // $plugin->kind = 5003; // TODO: remove unused
            $plugin->name = $this->name;
            $plugin->description = $this->description;
            $plugin->tos = $this->tos;
            $plugin->privacy = $this->privacy;
            $plugin->web = $this->web;
            $plugin->picture = $this->picture;
            $plugin->tags = json_encode($this->tags);
            //$plugin->mini_template = $this->generateMiniTemplate(); // TODO : remove unused
            $plugin->output_template =  json_encode([
                "output" => [
                    "title" => "Output",
                    "description" => "The output",
                    "type" => "string"
                ]
            ]); // TODO rename in output_sockets
            $plugin->input_template = $this->inputs->toJson(); // TODO rename in input_sockets
            $plugin->plugin_input = $this->plugin_input; // TODO rename to input_template

            $plugin->secrets = $this->secrets->toJson();
            $plugin->file_link = $this->file_link;
            $plugin->user_id = auth()->user()->id;
            $plugin->author =  auth()->user()->name;
            $plugin->payment = $this->payment;
            $plugin->wasm_upload = $wasm_upload->toJson();
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

    public function generateOutputTemplate()
    {
        return json_encode([
            'type' => $this->output_type,
            'description' => $this->output_description,
        ]);
    }

    public function addInput()
    {
        $this->inputs->push([
            'name' => '',
            'required' => false,
            'type' => 'string',
            'description' => '',
        ]);
    }

    public function removeInput($key)
    {
        $this->inputs->pull($key);
    }

    public function addSecretInput()
    {
        $this->secrets->push([
            'key' => '',
            'value' => '',
        ]);
    }

    public function removeSecretInput($key)
    {
        $this->secrets->pull($key);
    }

    public function render()
    {
        return view('livewire.plugins.plugin-create');
    }
}
