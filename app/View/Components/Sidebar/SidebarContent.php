<?php

namespace App\View\Components\Sidebar;

use App\Models\User;
use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;
use Illuminate\Support\Facades\Auth;

class SidebarContent extends Component
{
    public $recentThreads;

    /**
     * Create a new component instance.
     */
    public function __construct()
    {
        /** @var User $user */
        $user = Auth::user();
        $this->recentThreads = Auth::check() ? $user->threads()
            ->latest()
            ->take(10)
            ->get() : collect();
    }

    /**
     * Get the view / contents that represent the component.
     */
    public function render(): View|Closure|string
    {
        return view('components.sidebar.sidebar-content', [
            'recentThreads' => $this->recentThreads
        ]);
    }
}
