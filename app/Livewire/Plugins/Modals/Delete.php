<?php

namespace App\Livewire\Plugins\Modals;

use App\Livewire\Plugins\PluginList;
use App\Models\Plugin;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

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

            // Now delete the agent
            $plugin->delete();

            $this->dispatch('plugin_deleted')->to(PluginList::class);

            $this->closeModal();

            $this->alert('success', 'plugin deleted successfully');

        }

    }

    public function render()
    {
        return view('livewire.plugins.modals.delete');
    }
}
