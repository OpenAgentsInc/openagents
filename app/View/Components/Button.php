<?php

namespace App\View\Components;

use Illuminate\View\Component;

class Button extends Component
{
    public function __construct(
        public ?string $variant = 'default',
        public ?string $size = 'default',
        public ?string $tag = 'button',
        public ?bool $asChild = false
    ) {}

    public function render()
    {
        $baseClasses = 'inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50';

        $variantClasses = [
            'default' => 'bg-primary text-primary-foreground hover:bg-primary/90',
            'destructive' => 'bg-destructive text-destructive-foreground hover:bg-destructive/90',
            'outline' => 'border border-input bg-background hover:bg-accent hover:text-accent-foreground',
            'secondary' => 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
            'ghost' => 'hover:bg-accent hover:text-accent-foreground',
            'link' => 'text-primary underline-offset-4 hover:underline',
        ];

        $sizeClasses = [
            'default' => 'h-10 px-4 py-2',
            'sm' => 'h-9 rounded-md px-3',
            'lg' => 'h-11 rounded-md px-8',
            'icon' => 'h-10 w-10',
        ];

        $classes = $baseClasses . ' ' . 
                   ($variantClasses[$this->variant] ?? '') . ' ' . 
                   ($sizeClasses[$this->size] ?? '');

        return <<<HTML
            <{$this->tag} {{ \$attributes->merge(['class' => '$classes']) }}>
                {{ \$slot }}
            </{$this->tag}>
        HTML;
    }
}