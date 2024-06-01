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

class PluginEdit extends Component
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

    // public $outputs = [];

    public $file_link;

    // public $output_description;

    // public $output_type;

    public Collection $secrets;

    public $plugin_input;

    public $payment;

    public $user;

    public Plugin $plugin;

    public function mount()
    {

        if (! auth()->check()) {
            return redirect('/');
        }

        $user = auth()->user();

        abort_if($user->id !== $this->plugin->user_id, 403, 'permission denied');

        $this->user = $user;

        $this->setProperties();

    }

    public function rules()
    {
        return [
            'name' => 'required|string',
            'description' => 'required|string',
            'tos' => 'required|string',
            'privacy' => 'required|string',
            'author' => 'nullable|string',
            'payment' => 'nullable|string',
            'web' => 'nullable|string',
            // 'picture' => 'nullable|string',
            // 'tags' => 'required|array',
            // 'file_link' => ['required', 'string', 'url', 'active_url', new WasmUrl()],
            'wasm_upload' => ['nullable', 'file', new WasmFile()],
            'secrets' => 'nullable|array',
            'secrets.*.key' => 'required_with:secrets.*.value|string',
            'secrets.*.value' => 'required_with:secrets.*.key|string',
            'plugin_input' => 'required|string',
            'inputs' => 'required|array',
            'inputs.*.name' => 'required|string',
            'inputs.*.description' => 'required|string',
            'inputs.*.required' => 'required|boolean',
            'inputs.*.type' => 'required|string|in:string,integer,object,array',
            'output_description' => 'required|string',

        ];
    }

    public function submit()
    {

        // dd($this->inputs);

        $validated = $this->validate();

        // dd($validated);
        $good = false;

        try {

            $plugin = $this->plugin;

            if (! is_null($this->wasm_upload) || ! empty($this->wasm_upload)) {

                // delete old plugin first
                $oldfile = json_decode($plugin->wasm_upload);

                if ($oldfile && isset($oldfile->path)) {
                    Storage::disk($oldfile->disk)->delete($oldfile->path);
                }

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
            $plugin->input_template = $this->plugin_input;

            $plugin->secrets = $this->secrets->toJson();
            $plugin->file_link = $this->file_link;
            // $plugin->user_id = auth()->user()->id;
            // $plugin->author = auth()->user()->name;
            $plugin->payment = $this->payment;
            $plugin->save();

            $good = true;
        } catch (\Throwable $th) {
            Log::error('error from plugin : '.$th);
            $good = false;
        }

        if ($good) {
            $this->wasm_upload = null;
            $this->alert('success', 'Plugin successfully updated.');
            // return redirect()->route('plugins.index');
        } else {
            $this->alert('error', 'An error occured');
        }
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

    public function setProperties()
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
        $this->inputs = collect(json_decode($this->plugin->input_template, true));
        $this->secrets = collect(json_decode($this->plugin->secrets, true));
        $this->plugin_input = $this->plugin->plugin_input;
        $this->file_link = $this->plugin->file_link;
    }

    public function render()
    {
        return view('livewire.plugins.plugin-edit');
    }
}
