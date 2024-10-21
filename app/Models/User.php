<?php

namespace App\Models;

// use Illuminate\Contracts\Auth\MustVerifyEmail;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;

class User extends Authenticatable
{
    /** @use HasFactory<\Database\Factories\UserFactory> */
    use HasFactory, Notifiable;

    /**
     * The attributes that are mass assignable.
     *
     * @var array<int, string>
     */
    protected $fillable = [
        'name',
        'email',
        'password',
        'current_project_id',
        'current_team_id',
    ];

    /**
     * The attributes that should be hidden for serialization.
     *
     * @var array<int, string>
     */
    protected $hidden = [
        'password',
        'remember_token',
    ];

    /**
     * Get the attributes that should be cast.
     *
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'email_verified_at' => 'datetime',
            'password' => 'hashed',
        ];
    }

    public function teams(): BelongsToMany
    {
        return $this->belongsToMany(Team::class)->select(['teams.id', 'teams.name']);
    }

    public function projects(): HasMany
    {
        return $this->hasMany(Project::class);
    }

    public function threads(): HasMany
    {
        return $this->hasMany(Thread::class);
    }

    public function messages(): HasMany
    {
        return $this->hasMany(Message::class);
    }

    public function currentProject(): BelongsTo
    {
        return $this->belongsTo(Project::class, 'current_project_id');
    }

    public function currentTeam(): BelongsTo
    {
        return $this->belongsTo(Team::class, 'current_team_id');
    }

    public function createThread(array $data): Thread
    {
        $team_id = $data['team_id'] ?? $this->current_team_id ?? null;
        $project_id = $data['project_id'] ?? $this->current_project_id ?? null;

        if ($project_id) {
            $project = Project::findOrFail($project_id);
            if ($team_id && $project->team_id != $team_id) {
                throw new \InvalidArgumentException('The provided project does not belong to the specified team.');
            }
            $team_id = $project->team_id;
        }

        return $this->threads()->create([
            'title' => $data['title'] ?? 'New chat',
            'team_id' => $team_id,
            'project_id' => $project_id,
        ]);
    }
}