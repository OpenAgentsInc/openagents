<?php

namespace App\View\Components;

use Illuminate\View\Component;

class EmptyMessageList extends Component
{
    public function __construct()
    {
        //
    }

    public function render()
    {
        return view('components.empty-message-list');
    }
}