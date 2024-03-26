<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Thread extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function messages(): HasMany
    {
        return $this->hasMany(Message::class);
    }

    public function agents(): BelongsToMany
    {
        return $this->belongsToMany(Agent::class);
    }

    public function users(): BelongsToMany
    {
        return $this->belongsToMany(User::class);
    }

    public function setPrivate(): void
    {
        $this->update(['private' => true]);
    }

    public function setPublic(): void
    {
        $this->update(['private' => false]);
    }
}
