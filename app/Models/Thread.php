<?php

namespace App\Models;

use App\Models\CRM\Contact;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Thread extends Model
{
    /** @use HasFactory<\Database\Factories\ThreadFactory> */
    use HasFactory;

    protected $fillable = [
        'title',
        'project_id',
        'user_id',
    ];

    public function project(): BelongsTo
    {
        return $this->belongsTo(Project::class);
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    public function messages(): HasMany
    {
        return $this->hasMany(Message::class);
    }

    public function contacts(): BelongsToMany
    {
        return $this->belongsToMany(Contact::class);
    }

    /**
     * Get the thread's context, including project context.
     */
    public function getContext(): string
    {
        if (!$this->project) {
            return '';
        }

        return $this->project->getContext();
    }

    /**
     * Get the thread's instructions, including project instructions.
     */
    public function getInstructions(): string
    {
        if (!$this->project) {
            return '';
        }

        return $this->project->getInstructions();
    }
}