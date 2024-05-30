<?php

namespace App\Livewire\Plugins\Modals;

use App\Models\Plugin;
use App\Livewire\Plugins\PluginList;
use LivewireUI\Modal\ModalComponent;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;

class Delete extends ModalComponent
{
    use LivewireAlert;

    public $plugin;

    public $name;

    public $plugin_id;

    public function mount(Plugin $plugin)
    {
        $this->name = $plugin->name;
        $this->plugin_id = $plugin->id;
    }

    public function delete()
    {

        $plugin = Plugin::with('user')->find($this->plugin_id);


        if($plugin && $plugin->user_id !== auth()->user()->id) {

            $this->alert('error', 'Permission Denied..');
            $this->closeModal();

        } else {

            //delete the file first
            $oldfile = json_decode($plugin->wasm_upload);

            if ($oldfile && isset($oldfile->path)) {
                Storage::disk($oldfile->disk)->delete($oldfile->path);
            }


            // Now delete the agent
            $plugin->delete();

            $this->dispatch('plugin_updated');

            $this->dispatch('plugin_updated')->to(PluginList::class);

            $this->closeModal();

            $this->alert('success', 'plugin deleted successfully');





        }

    }

    public function render()
    {
        return view('livewire.plugins.modals.delete');
    }
}
