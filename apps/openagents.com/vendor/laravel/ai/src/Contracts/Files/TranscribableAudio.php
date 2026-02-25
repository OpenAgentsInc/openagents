<?php

namespace Laravel\Ai\Contracts\Files;

use Laravel\Ai\PendingResponses\PendingTranscriptionGeneration;
use Stringable;

interface TranscribableAudio extends HasContent, HasMimeType, Stringable
{
    /**
     * Generate a transcription of the given audio.
     */
    public function transcription(): PendingTranscriptionGeneration;
}
