<?php

namespace App\Models;

use App\Traits\Payable;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;
    use Payable;

    protected $fillable = [
        'name',
        'description',
        'tos',
        'privacy',
        'web',
        'picture',
        'tags',
        'input_sockets',
        'output_sockets',
        'input_template',
        'secrets',
        'file_link',
        'user_id',
        'author',
        'payment',
        'wasm_upload',
        'allowed_hosts',
        'enabled',
        'suspended',
        'pending_revision',
        'pending_revision_reason',
        'price_msats',

    ];

    protected $casts = [
        'tags' => 'json',
        'secrets' => 'json',
        'wasm_upload' => 'json',

    ];

    protected $appends = ['image_url'];

    public function getImageUrlAttribute()
    {
        if (is_null($this->picture) || empty($this->picture)) {  // Check if $this->image is null
            return url('/images/sqlogo.png'); // Return default URL if null
        }
        $imageData = json_decode($this->picture, true); // Cast to array for access
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

    public function user()
    {
        return $this->belongsTo(User::class);
    }

    public function isEditableBy($user)
    {

        if ($this->user->id == $user->id) {
            return true;
        }

        $pluginAuthor = $this->user;

        return $user->getRole()->canModerate($pluginAuthor->getRole());
    }

    public function agents()
    {
        return $this->belongsToMany(Agent::class);
    }
}
