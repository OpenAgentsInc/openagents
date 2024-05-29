<?php

namespace App\Models;

use App\Traits\Payable;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Support\Facades\Storage;

class Agent extends Model
{
    use HasFactory, Payable;

    protected $appends = ['sats_earned', 'image_url', 'thread_count', 'unique_users_count', 'creator_username'];

    protected $fillable = [
        'name',
        'image',
        'about',
        'message',
        'prompt',
        'rag_prompt',
        'is_public',
        'user_id',
        'is_rag_ready',
        'use_tools',
    ];

    public function getCreatorPictureAttribute()
    {
        return $this->user->profile_photo_path ?? '/images/nostrich.jpeg';
    }

    public function getCreatorUsernameAttribute()
    {
        if (! $this->user) {
            return 'Unknown';
        }

        // If there's a user, return the username, otherwise assume Nostr user and use name
        return $this->user->username ?? $this->user->name;
    }

    public function getImageUrlAttribute()
    {
        if (is_null($this->image) || empty($this->image)) {  // Check if $this->image is null
            //            dd('null for image for agent'.$this->name);

            return url('/images/sqlogo.png'); // Return default URL if null
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

        return url('/images/sqlogo.png'); // Return default URL if no image data or empty URL
    }

    public function documents()
    {
        return $this->hasMany(AgentFile::class);
    }

    public function user()
    {
        return $this->belongsTo(User::class);
    }

    public function getThreadCount()
    {
        return $this->messages()->distinct('thread_id')->count('thread_id');
    }

    public function messages()
    {
        return $this->hasMany(Message::class);
    }

    public function hasCapability($capability)
    {
        // Agent has JSON blob in "capabilities" field
        $capabilities = json_decode($this->capabilities, true);

        // If capabilities is null, return false
        if (is_null($capabilities)) {
            return false;
        }

        // If the capability is in the array, return true
        return in_array($capability, $capabilities);
    }

    // In Agent.php

    //    public function getUniqueUsersCountAttribute()
    //    {
    //        return $this->threads()->distinct('user_id')->count('user_id');
    //    }

    public function getUniqueUsersCountAttribute()
    {
        return $this->messages()->distinct('user_id')->count('user_id');
    }

    public function threads()
    {
        return $this->hasMany(Thread::class);
    }

    public function getThreadCountAttribute()
    {
        return $this->messages()->distinct('thread_id')->count('thread_id');
    }
}
