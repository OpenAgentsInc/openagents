<?php

namespace App\View\Components\Sidebar;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;
use Illuminate\Support\Facades\Auth;

class SidebarNew extends Component
{
    public $recentThreads;

    /**
     * Create a new component instance.
     */
    public function __construct()
    {
        $this->recentThreads = Auth::check() ? Auth::user()->threads()
            ->latest()
            ->take(10)
            ->get() : collect();
    }

    /**
     * Get the view / contents that represent the component.
     */
    public function render(): View|Closure|string
    {
        return view('components.sidebar.sidebar-new');
    }
}