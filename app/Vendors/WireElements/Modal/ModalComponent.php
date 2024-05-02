<?php

namespace App\Vendors\WireElements\Modal;

use InvalidArgumentException;
use Livewire\Component;
use LivewireUI\Modal\Contracts\ModalComponent as Contract;
use LivewireUI\Modal\ModalComponent as BaseModalComponent;

abstract class ModalComponent extends BaseModalComponent
{
    public bool $forceClose = false;

    public int $skipModals = 0;

    public bool $destroySkipped = false;

    protected static array $maxWidths = [
        'auth' => 'sm:max-w-sm md:max-w-[600px]',
        'explore' => 'max-w-[900px]',
        'sm' => 'sm:max-w-sm',
        'md' => 'sm:max-w-md',
        'lg' => 'sm:max-w-md md:max-w-lg',
        'xl' => 'sm:max-w-md md:max-w-xl',
        '2xl' => 'sm:max-w-md md:max-w-xl lg:max-w-2xl',
        '3xl' => 'sm:max-w-md md:max-w-xl lg:max-w-3xl',
        '4xl' => 'sm:max-w-md md:max-w-xl lg:max-w-3xl xl:max-w-4xl',
        '5xl' => 'sm:max-w-md md:max-w-xl lg:max-w-3xl xl:max-w-5xl',
        '6xl' => 'sm:max-w-md md:max-w-xl lg:max-w-3xl xl:max-w-5xl 2xl:max-w-6xl',
        '7xl' => 'sm:max-w-md md:max-w-xl lg:max-w-3xl xl:max-w-5xl 2xl:max-w-7xl',
    ];

    public static function modalMaxWidth(): string
    {
        return config('wire-elements-modal.component_defaults.modal_max_width', '2xl');
    }

    public static function modalMaxWidthClass(): string
    {
        if (! array_key_exists(static::modalMaxWidth(), static::$maxWidths)) {
            throw new InvalidArgumentException(
                sprintf('Modal max width [%s] is invalid. The width must be one of the following [%s].',
                    static::modalMaxWidth(), implode(', ', array_keys(static::$maxWidths))),
            );
        }

        return static::$maxWidths[static::modalMaxWidth()];
    }
}
