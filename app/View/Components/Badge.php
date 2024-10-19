<?php

namespace App\View\Components;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class Badge extends Component
{
    public string $uuid;

    public function __construct(
        public ?string $value = null,
        public string $variant = 'default'
    ) {
        $this->uuid = "oa" . md5(serialize($this));
    }

    public function render(): View|Closure|string
    {
        $baseClasses = "inline-flex items-center rounded-md border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2";

        $variantClasses = [
            'default' => 'border-transparent bg-primary text-primary-foreground shadow hover:bg-primary/80',
            'secondary' => 'border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80',
            'destructive' => 'border-transparent bg-destructive text-destructive-foreground shadow hover:bg-destructive/80',
            'outline' => 'text-foreground',
        ];

        $classes = $baseClasses . ' ' . ($variantClasses[$this->variant] ?? $variantClasses['default']);

        return <<<HTML
            <div {{ \$attributes->merge(['class' => "{$classes}"]) }}>
                {{ \$value }}
            </div>
        HTML;
    }
}
