<?php

class ImageService
{
    public function addImageToThread($image, $thread)
    {
        // Read the image file contents
        $imageContents = $image->get();

        // Encode the image contents to base64
        $imageBase64 = base64_encode($imageContents);
        dd($imageBase64);

        //        // Save the image to the storage disk
        //        $path = $image->store('public/images');
        //
        //        // Create a new message with the image path
        //        $thread->messages()->create([
        //            'body' => $path,
        //            'session_id' => $thread->session_id,
        //            'model' => $thread->model,
        //            'user_id' => auth()->id() ?? null,
        //        ]);
    }
}
