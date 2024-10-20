<?php

namespace App\View\Components;

use Illuminate\View\Component;

class DropdownItem extends Component
{
    public function __construct(
        public $value,
        public bool $selected = false
    ) {}

    public function render()
    {
        return function (array $data) {
            return <<<'BLADE'
                <a href="#" class="block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer" role="menuitem">
                    {{ $slot }}
                </a>
            BLADE;
        };
    }
}