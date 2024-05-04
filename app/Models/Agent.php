<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Support\Facades\Storage;

class Agent extends Model
{
    use HasFactory;

    protected $fillable = [
        'name',
        'image',
        'about',
        'message',
        'prompt',
        'rag_prompt',
        'is_public',
        'user_id',
        'is_rag_ready'
    ];

    public function getImageUrlAttribute()
    {
        if (is_null($this->image) || empty($this->image)) {  // Check if $this->image is null
            return url('/images/no-image.jpg'); // Return default URL if null
        }
        $imageData = json_decode($this->image, true); // Cast to array for access
        if ($imageData && isset($imageData['url']) && ! empty($imageData['url'])) {  // Check for non-empty URL
            $url = $imageData['url'];

            // If it starts with "/storage" then it's a local file and we need to resolve the path
            if (strpos($url, '/storage') === 0) {
                //                dd($url);
                // But first remove the /storage at the beginning
                $url = substr($url, 9);

                return url(Storage::url($url));
            } else {
                return $url;

            }

            //            return $imageData['url'];
        }

        return url('/images/no-image.jpg'); // Return default URL if no image data or empty URL
    }

    public function documents()
    {
        return $this->hasMany(AgentFile::class);
    }

    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
