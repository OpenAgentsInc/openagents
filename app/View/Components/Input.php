<?php

namespace App\View\Components;

use Illuminate\View\Component;
use Illuminate\View\View;

class Input extends Component
{
    public function __construct(
        public ?string $label = null,
        public ?string $type = 'text',
        public ?string $name = null,
        public ?string $id = null,
        public ?string $placeholder = null,
        public ?string $value = null,
        public bool $required = false,
        public ?string $icon = null,
    ) {}

    public function render(): View
    {
        return view('components.input');
    }
}