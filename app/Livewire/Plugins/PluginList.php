<?php

namespace App\Livewire\Plugins;

use App\Models\User;
use App\Models\Plugin;
use Livewire\Component;
use Livewire\Attributes\Url;
use Livewire\Attributes\Computed;
use Illuminate\Support\Facades\Auth;

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
        ->with('user')
        ->latest()
            ->paginate(12);
    }

    #[On('plugin_updated')]
    public function index()
    {
        $this->plugins();
    }

    public function render()
    {
        return view('livewire.plugins.plugin-list');
    }
}
