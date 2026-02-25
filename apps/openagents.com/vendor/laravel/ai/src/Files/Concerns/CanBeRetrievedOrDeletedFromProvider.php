<?php

namespace Laravel\Ai\Files\Concerns;

use Laravel\Ai\Files;
use Laravel\Ai\Responses\FileResponse;

trait CanBeRetrievedOrDeletedFromProvider
{
    /**
     * Store the file on a given provider.
     */
    public function get(?string $provider = null): FileResponse
    {
        return Files::get($this->id, provider: $provider);
    }

    /**
     * Delete the file on a given provider.
     */
    public function delete(?string $provider = null): void
    {
        Files::delete($this->id, provider: $provider);
    }
}
