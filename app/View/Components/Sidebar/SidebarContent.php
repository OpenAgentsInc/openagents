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
    public $projects;

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

        // Demo project data
        $this->projects = [
            [
                'title' => 'Design Engineering',
                'icon' => 'frame',
            ],
            [
                'title' => 'Sales & Marketing',
                'icon' => 'pie-chart',
            ],
            [
                'title' => 'Travel',
                'icon' => 'map',
            ],
        ];

        // TODO: Replace with actual user projects from database
        // $this->projects = Auth::check() ? $user->projects()->get() : collect();
    }

    /**
     * Get the view / contents that represent the component.
     */
    public function render(): View|Closure|string
    {
        return view('components.sidebar.sidebar-content', [
            'recentThreads' => $this->recentThreads,
            'projects' => $this->projects,
        ]);
    }
}