<?php

namespace App\View\Components;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class Chatbox extends Component
{
    public $autoscroll;

    /**
     * Create a new component instance.
     */
    public function __construct($autoscroll = true)
    {
        $this->autoscroll = $autoscroll;
    }

    /**
     * Get the view / contents that represent the component.
     */
    public function render(): View|Closure|string
    {
        return view('components.chatbox', [
            'autoscroll' => $this->autoscroll,
        ]);
    }
}
