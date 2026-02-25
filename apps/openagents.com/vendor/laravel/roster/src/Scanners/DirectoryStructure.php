<?php

namespace Laravel\Roster\Scanners;

use Illuminate\Support\Collection;
use Laravel\Roster\Approach;
use Laravel\Roster\Enums\Approaches;

class DirectoryStructure
{
    public function __construct(protected string $path) {}

    /**
     * @return \Illuminate\Support\Collection<int, \Laravel\Roster\Package|\Laravel\Roster\Approach>
     */
    public function scan(): Collection
    {
        $items = collect();

        $actions = $this->path.DIRECTORY_SEPARATOR.'/app/'.DIRECTORY_SEPARATOR.'Actions';
        if (is_dir($actions)) {
            $items->push(new Approach(Approaches::ACTION));
        }

        $domains = $this->path.DIRECTORY_SEPARATOR.'/app/'.DIRECTORY_SEPARATOR.'Domains';
        if (is_dir($domains)) {
            $items->push(new Approach(Approaches::DDD));
        }

        if (is_dir($this->path.DIRECTORY_SEPARATOR.'modules') || is_dir($this->path.DIRECTORY_SEPARATOR.'Modules') || is_dir($this->path.DIRECTORY_SEPARATOR.'app-modules')) {
            $items->push(new Approach(Approaches::MODULAR));
        }

        return $items;
    }
}
