<?php

namespace App\Services;

use App\AI\ImageInferencer;

class ImageService
{
    public function getImageDescription($image, $thread): string
    {
        // Read the image file contents
        $imageContents = $image->get();

        // Get the file path
        $originalFilename = $image->getClientOriginalName();

        // Encode the image contents to base64
        $imageBase64 = base64_encode($imageContents);

        // Create a JSON input for the inferencer
        $input = json_encode([
            'text' => '',
            'images' => [$imageBase64],
        ]);

        // Get the model and stream function from the thread
        $model = $thread->model;
        $streamFunction = function ($response) {
            // You can add logic here to handle the stream response
        };

        // Use the ImageInferencer to get a summary of the image
        $inference = ImageInferencer::multimodalInference($input, $model, $streamFunction);

        // Extract the summary from the inference response
        $summary = "The user has uploaded an image: `$originalFilename` which shows the following:\n\n";

        return $summary.$inference['content'];
    }
}
