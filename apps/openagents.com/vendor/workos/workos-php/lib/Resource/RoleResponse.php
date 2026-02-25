<?php

namespace WorkOS\Resource;

/**
 * Class RoleResponse.
 *
 * @property string $slug
 */
class RoleResponse
{
    public $slug;

    public function __construct(string $slug)
    {
        $this->slug = $slug;
    }
}
