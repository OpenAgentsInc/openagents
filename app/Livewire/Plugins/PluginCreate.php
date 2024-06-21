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

    protected $listeners = [
        'tags-updated' => 'updateTags',
        'confirm-suspension' => 'confirmedSuspend',
        'confirm-rejection' => 'confirmedReject',
    ];

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

    public $filename = '';

    public $user;

    public Plugin $plugin;

    public $suspended = '';

    public $pending_revision_reason = '';

    public $enabled = true;

    public $cost_sats = 0;

    private function checkPermissions(): bool
    {
        if (! auth()->check()) {
            return false;
        }
        $user = auth()->user();
        if (isset($this->plugin)) {
            if (! $this->plugin->isEditableBy($user)) {
                return false;
            }
        }

        return true;
    }

    public function mount()
    {

        if (! $this->checkPermissions()) {
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
            $this->loadPluginProperties();
        }

    }

    public function loadPluginProperties()
    {
        if (isset($this->plugin->pending_revision) && ! empty($this->plugin->pending_revision)) {
            $revision = json_decode($this->plugin->pending_revision, true);
        } else {
            $revision = [];
        }

        $this->name = $revision['name'] ?? $this->plugin->name;
        $this->description = $revision['description'] ?? $this->plugin->description;
        $this->tos = $revision['tos'] ?? $this->plugin->tos;
        $this->privacy = $revision['privacy'] ?? $this->plugin->privacy;
        $this->payment = $revision['payment'] ?? $this->plugin->payment;
        $this->web = $revision['web'] ?? $this->plugin->web;
        $this->tags = $revision['tags'] ?? $this->plugin->tags;
        $this->inputs = $revision['inputs'] ?? json_decode($this->plugin->input_sockets, true);
        $this->secrets = $revision['secrets'] ?? json_decode($this->plugin->secrets, true);
        $this->input_template = $revision['input_template'] ?? $this->plugin->input_template;
        $this->file_link = $revision['file_link'] ?? $this->plugin->file_link;
        $this->enabled = $revision['enabled'] ?? $this->plugin->enabled;
        $this->suspended = $this->plugin->suspended;
        $this->pending_revision_reason = $this->plugin->pending_revision_reason;
        $this->allowed_hosts = $revision['allowed_hosts'] ?? json_decode($this->plugin->allowed_hosts ?? '[]', true);
        $this->tags = $revision['tags'] ?? json_decode($this->plugin->tags ?? '[]', true);
        $this->cost_sats = $revision['cost_sats'] ?? $this->plugin->price_msats / 1000;

        // HOTFIX: if not array reset to empty array
        if (! is_array($this->tags)) {
            $this->tags = [];
        }
    }

    public function rules()
    {
        return [
            'name' => 'required|string',
            'description' => 'required|string',
            'tos' => 'nullable|string',
            'privacy' => 'nullable|string',
            'payment' => 'nullable|string',
            'web' => 'nullable|string',
            'picture' => 'nullable|image|max:2048',
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
            'tags' => 'nullable|array',
            'tags.*' => 'required|string',
            'enabled' => 'required|boolean',
            'cost_sats' => 'nullable|integer',
        ];
    }

    public function suspend()
    {
        $this->dispatch('show-input-string-dialog', [
            'confirmFunction' => 'confirm-suspension',
            'title' => 'Please enter a reason for suspension',
        ]);

    }

    public function confirmedSuspend($reason)
    {
        $user = auth()->user();
        if (! $user->isModerator()) {
            $this->alert('error', 'Permission denied');

            return;
        }
        $this->plugin->update([
            'suspended' => $reason,
        ]);
        $this->dispatch('refresh');

    }

    public function reject()
    {
        $this->dispatch('show-input-string-dialog', [
            'confirmFunction' => 'confirm-rejection',
            'title' => 'Please enter a reason for rejection',
        ]);
    }

    public function confirmedReject($reason)
    {
        $user = auth()->user();
        if (! $user->isModerator()) {
            $this->alert('error', 'Permission denied');

            return;
        }
        if ($this->plugin->pending_revision) {
            $this->plugin->update([
                'pending_revision_reason' => $reason,
            ]);
        } else {
            $this->plugin->update([
                'suspended' => $reason,
            ]);
        }
        redirect()->refresh();
    }

    public function submit()
    {
        $validated = $this->validate();

        $saveimage = null;

        $plugin = null;

        $update = true;
        if (! isset($this->plugin)) {
            $plugin = new Plugin();
            $update = false;
            $plugin->user_id = auth()->user()->id;
        } else {
            abort_if(! $this->checkPermissions(), 403, 'permission denied');
            $plugin = $this->plugin;
            $saveimage = json_decode($this->plugin->picture);
        }

        if (! is_null($this->picture) || ! empty($this->picture)) {

            if ($update) {
                $oldimage = json_decode($this->plugin->picture);

                if ($oldimage && isset($oldimage->path)) {
                    $path = $oldimage->path;
                    $disk = $oldimage->disk;
                    Storage::disk($disk)->delete($path);
                }
            }

            $disk = config('filesystems.media_disk');

            // Get filename with extension
            $filenamewithextension = $this->picture->getClientOriginalName();

            // Get filename without extension
            $filename = pathinfo($filenamewithextension, PATHINFO_FILENAME);

            // Get file extension
            $extension = $this->picture->getClientOriginalExtension();

            // Filename to store with directory
            $imagenametostore = 'plugins/profile/images/'.str($filename)->slug()->toString().'_'.time().'.'.$extension;

            // Upload File to public
            Storage::disk($disk)->put($imagenametostore, fopen($this->picture->getRealPath(), 'r+'), 'public');

            $saveimage = [
                'disk' => $disk,
                'path' => $imagenametostore,
                'url' => Storage::disk($disk)->url($imagenametostore),
            ];
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
                'name' => $filenameWithExt,
            ]);
            $this->file_link = $url;
        } elseif (! $update) {
            $this->alert('error', 'Wasm file is required');

            return;
        }

        try {
            $user = auth()->user();

            if ($user->isModerator() || ! $update) {
                $plugin->name = $this->name;
                $plugin->description = $this->description;
                $plugin->tos = $this->tos ?? '';
                $plugin->privacy = $this->privacy ?? '';
                $plugin->web = $this->web ?? '';
                $plugin->price_msats = ($this->cost_sats ?? 0) * 1000;

                if ($this->picture) {
                    $plugin->picture = json_encode($saveimage);
                }
                // $plugin->tags = json_encode($this->tags);
                $plugin->tags = isset($this->tags) ? json_encode($this->tags) : json_encode([]);
                $plugin->output_sockets = json_encode([
                    'output' => [
                        'name' => 'Output',
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

                $plugin->enabled = $this->enabled;

                $plugin->pending_revision = '';
                $plugin->pending_revision_reason = '';
                if (! $user->isModerator()) {
                    $plugin->suspended = 'Pending approval';
                } else {
                    $plugin->suspended = '';
                    $plugin->short_description = '';
                }

                $plugin->save();
            } else {
                $plugin->pending_revision = json_encode([
                    'name' => $this->name,
                    'description' => $this->description,
                    'tos' => $this->tos ?? '',
                    'privacy' => $this->privacy ?? '',
                    'web' => $this->web ?? '',
                    'picture' => $this->picture,
                    'tags' => $this->tags,
                    'inputs' => $this->inputs,
                    'secrets' => $this->secrets,
                    'input_template' => $this->input_template,
                    'file_link' => $this->file_link,
                    'payment' => $this->payment,
                    'allowed_hosts' => $this->allowed_hosts,
                    'enabled' => $this->enabled,
                    'cost_sats' => $this->cost_sats,

                ]);
                $plugin->pending_revision_reason = 'Pending approval';
                $plugin->save();
            }

            $this->alert('success', 'Success');

            if ($update) {
                $this->dispatch('refresh');
            } else {
                return redirect()->route('plugins.index', ['plugin' => $plugin]);
            }

        } catch (\Throwable $th) {
            Log::error('error forom plugin : '.$th);
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
