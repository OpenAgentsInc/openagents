<?php

namespace App\Models\CRM;

use App\Models\Team;
use App\Models\User;
use App\Models\Thread;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Contact extends Model
{
    /** @use HasFactory<\Database\Factories\CRM\ContactFactory> */
    use HasFactory;

    protected $fillable = [
        'contact_id',
        'company_id',
        'team_id',
        'created_by',
        'first_name',
        'last_name',
        'email',
        'phone',
        'title',
        'note',
    ];

    protected static function boot()
    {
        parent::boot();
        
        static::creating(function ($contact) {
            if (!$contact->contact_id) {
                $contact->contact_id = 'CT' . str_pad(random_int(1, 999999), 6, '0', STR_PAD_LEFT);
            }
        });
    }

    public function company(): BelongsTo
    {
        return $this->belongsTo(Company::class);
    }

    public function teams(): BelongsToMany
    {
        return $this->belongsToMany(Team::class);
    }

    public function activities(): HasMany
    {
        return $this->hasMany(Activity::class);
    }

    public function notes(): HasMany
    {
        return $this->hasMany(Note::class);
    }

    public function tags(): BelongsToMany
    {
        return $this->belongsToMany(Tag::class);
    }

    public function threads(): BelongsToMany
    {
        return $this->belongsToMany(Thread::class);
    }

    public function creator(): BelongsTo
    {
        return $this->belongsTo(User::class, 'created_by');
    }

    public function getFormattedPhoneAttribute(): string
    {
        if (!$this->phone) {
            return '';
        }

        $phone = preg_replace('/[^0-9]/', '', $this->phone);
        return '(' . substr($phone, 0, 3) . ') ' . substr($phone, 3, 3) . '-' . substr($phone, 6);
    }

    public function calculateEngagementScore(): float
    {
        $score = 0;
        
        // Add points for activities
        $score += $this->activities()->count() * 10;
        
        // Add points for chat threads
        $score += $this->threads()->count() * 5;
        
        return $score;
    }
}