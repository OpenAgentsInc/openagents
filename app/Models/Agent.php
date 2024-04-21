<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

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
    ];




    public function getImageUrlAttribute()
    {
        if (is_null($this->image) || empty($this->image)) {  // Check if $this->image is null
            return url('/images/no-image.jpg'); // Return default URL if null
        }
        $imageData = json_decode($this->image, true); // Cast to array for access
        if ($imageData && isset($imageData['url']) && !empty($imageData['url'])) {  // Check for non-empty URL
            return $imageData['url'];
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
