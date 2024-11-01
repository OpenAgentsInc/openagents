<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Support\Facades\Gate;
use Illuminate\Validation\ValidationException;

class Project extends Model
{
    /** @use HasFactory<\Database\Factories\ProjectFactory> */
    use HasFactory;

    protected $fillable = [
        'name',
        'description',
        'user_id',
        'team_id',
        'is_default',
        'custom_instructions',
        'context',
        'settings',
        'status',
    ];

    protected $casts = [
        'is_default' => 'boolean',
        'settings' => 'array',
    ];

    /**
     * The "booted" method of the model.
     */
    protected static function booted(): void
    {
        static::creating(function ($project) {
            if (!$project->name) {
                throw ValidationException::withMessages([
                    'name' => ['The project name is required.'],
                ]);
            }

            // Check name uniqueness within scope
            $query = static::query()
                ->where('name', $project->name);

            if ($project->team_id) {
                $query->where('team_id', $project->team_id);
            } elseif ($project->user_id) {
                $query->where('user_id', $project->user_id);
            }

            if ($query->exists()) {
                throw ValidationException::withMessages([
                    'name' => ['A project with this name already exists in this scope.'],
                ]);
            }
        });
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    public function team(): BelongsTo
    {
        return $this->belongsTo(Team::class);
    }

    public function threads(): HasMany
    {
        return $this->hasMany(Thread::class);
    }

    public function files(): HasMany
    {
        return $this->hasMany(File::class);
    }

    /**
     * Archive the project.
     */
    public function archive(): void
    {
        $this->status = 'archived';
        $this->save();
    }

    /**
     * Determine if the user can access the project.
     */
    public function canBeAccessedBy(User $user): bool
    {
        if ($this->user_id === $user->id) {
            return true;
        }

        if ($this->team_id && $user->teams()->where('teams.id', $this->team_id)->exists()) {
            return true;
        }

        return false;
    }

    /**
     * Get the project's context for threads.
     */
    public function getContext(): string
    {
        return $this->context ?? '';
    }

    /**
     * Get the project's instructions for threads.
     */
    public function getInstructions(): string
    {
        return $this->custom_instructions ?? '';
    }
}