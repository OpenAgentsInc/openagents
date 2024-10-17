<?php

namespace App\View\Components;

use Illuminate\View\Component;

class Card extends Component
{
    public function __construct(
        public ?string $class = ''
    ) {}

    public function render()
    {
        $baseClasses = 'rounded-xl border bg-card text-card-foreground shadow';
        $classes = $baseClasses . ' ' . $this->class;

        return <<<HTML
            <div {{ \$attributes->merge(['class' => '$classes']) }}>
                {{ \$slot }}
            </div>
        HTML;
    }
}

class CardHeader extends Component
{
    public function __construct(
        public ?string $class = ''
    ) {}

    public function render()
    {
        $baseClasses = 'flex flex-col space-y-1.5 p-6';
        $classes = $baseClasses . ' ' . $this->class;

        return <<<HTML
            <div {{ \$attributes->merge(['class' => '$classes']) }}>
                {{ \$slot }}
            </div>
        HTML;
    }
}

class CardTitle extends Component
{
    public function __construct(
        public ?string $class = ''
    ) {}

    public function render()
    {
        $baseClasses = 'font-semibold leading-none tracking-tight';
        $classes = $baseClasses . ' ' . $this->class;

        return <<<HTML
            <h3 {{ \$attributes->merge(['class' => '$classes']) }}>
                {{ \$slot }}
            </h3>
        HTML;
    }
}

class CardDescription extends Component
{
    public function __construct(
        public ?string $class = ''
    ) {}

    public function render()
    {
        $baseClasses = 'text-sm text-muted-foreground';
        $classes = $baseClasses . ' ' . $this->class;

        return <<<HTML
            <p {{ \$attributes->merge(['class' => '$classes']) }}>
                {{ \$slot }}
            </p>
        HTML;
    }
}

class CardContent extends Component
{
    public function __construct(
        public ?string $class = ''
    ) {}

    public function render()
    {
        $baseClasses = 'p-6 pt-0';
        $classes = $baseClasses . ' ' . $this->class;

        return <<<HTML
            <div {{ \$attributes->merge(['class' => '$classes']) }}>
                {{ \$slot }}
            </div>
        HTML;
    }
}

class CardFooter extends Component
{
    public function __construct(
        public ?string $class = ''
    ) {}

    public function render()
    {
        $baseClasses = 'flex items-center p-6 pt-0';
        $classes = $baseClasses . ' ' . $this->class;

        return <<<HTML
            <div {{ \$attributes->merge(['class' => '$classes']) }}>
                {{ \$slot }}
            </div>
        HTML;
    }
}