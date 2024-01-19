<?php

namespace App\View\Components;

use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class PluginGrid extends Component
{
    public $plugins;

    /**
     * Create a new component instance.
     *
     * @param  mixed  $plugins
     */
    public function __construct($plugins)
    {
        $this->plugins = $plugins;
    }

    /**
     * Get the view / contents that represent the component.
     */
    public function render(): View
    {
        return view('components.plugin-grid');
    }
}
