<?php

namespace App\View\Components;

use Illuminate\View\Component;

class Button extends Component
{
    public $variant;
    public $size;
    public $asChild;

    public function __construct($variant = 'default', $size = 'default', $asChild = false)
    {
        $this->variant = $variant;
        $this->size = $size;
        $this->asChild = $asChild;
    }

    public function render()
    {
        return view('components.button');
    }

    public function classes()
    {
        $baseClasses = 'button';
        $variantClasses = [
            'default' => 'button--default',
            'destructive' => 'button--destructive',
            'outline' => 'button--outline',
            'secondary' => 'button--secondary',
            'ghost' => 'button--ghost',
            'link' => 'button--link',
        ];
        $sizeClasses = [
            'default' => 'button--size-default',
            'sm' => 'button--size-sm',
            'lg' => 'button--size-lg',
            'icon' => 'button--size-icon',
        ];

        return $baseClasses . ' ' . 
               ($variantClasses[$this->variant] ?? '') . ' ' . 
               ($sizeClasses[$this->size] ?? '');
    }
}