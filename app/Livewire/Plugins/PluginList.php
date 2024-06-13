<?php

namespace App\Livewire\Plugins;

use App\Models\Plugin;
use Livewire\Attributes\Computed;
use Livewire\Attributes\On;
use Livewire\Attributes\Url;
use Livewire\Component;

class PluginList extends Component
{
    #[Url(except: '', as: 'q')]
    public $search = '';

    #[Computed]
    public function plugins()
    {

        return Plugin::query()->when($this->search, function ($query) {
            return $query->where('name', 'like', '%'.$this->search.'%');
        })
            ->where('suspended', '')
            ->with('user')
            ->latest()
            ->paginate(12);
    }

    #[Computed]
    public function suspendedPlugins()
    {
        $user = auth()->user();

        return Plugin::query()
            ->whereNot('suspended', '')
            ->with('user')
            ->latest()
            ->get()
            ->filter(function ($plugin) use ($user) {
                return $plugin->isEditableBy($user);
            });
    }

    #[On('plugin_updated')]
    public function index()
    {
        $this->plugins();
    }

    public function render()
    {
        return view('livewire.plugins.plugin-list', ['plugins' => $this->plugins()]);
    }
}
