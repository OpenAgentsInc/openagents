<?php

namespace App\View\Components;

use Illuminate\View\Component;
use Illuminate\Support\Str;

class Dropdown extends Component
{
    public string $uuid;

    public function __construct(
        public string $label,
        public array $items,
        public ?string $id = null,
        public bool $checkable = false,
        public ?string $icon = null,
        public ?string $class = '',
        public bool $loading = false,
        public ?string $loadingLabel = null,
        public $selected = null
    ) {
        $this->uuid = $id ?? Str::uuid();
    }

    public function render()
    {
        return function (array $data) {
            return view('components.dropdown', $data);
        };
    }
}