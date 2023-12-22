<?php

namespace App\Models;

// use Illuminate\Contracts\Auth\MustVerifyEmail;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;
use Laravel\Sanctum\HasApiTokens;

class User extends Authenticatable
{
    use HasApiTokens, HasFactory, Notifiable;

    // When user is created, check session for an r variable - then look up the user by username and set the referrer_id
    protected static function booted()
    {
        static::creating(function ($user) {
            if (session()->has('r')) {
                $referrer = User::where('github_nickname', session('r'))->first();
                if ($referrer) {
                    $user->referrer_id = $referrer->id;
                }
            }
        });
    }

    /**
     * The attributes that are mass assignable.
     *
     * @var array<int, string>
     */
    protected $fillable = [
        'name',
        'email',
        'password',
        'github_id',
        'github_nickname',
        'github_avatar',
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
     * The attributes that should be cast.
     *
     * @var array<string, string>
     */
    protected $casts = [
        'email_verified_at' => 'datetime',
        'password' => 'hashed',
    ];


    public function agents()
    {
        return $this->hasMany(Agent::class);
    }

    public function conversations()
    {
        return $this->hasMany(Conversation::class);
    }

    public function files()
    {
        return $this->hasMany(File::class);
    }

    public function messages()
    {
        return $this->hasMany(Message::class);
    }

    public function pay(int $amount)
    {
        $this->balance += $amount;
        $this->save();
    }

    public function getUsernameAttribute()
    {
        return $this->github_nickname;
    }

    public function referrer()
    {
        return $this->belongsTo(User::class);
    }

    // referrals are users that have this user as their referrer
    public function referrals()
    {
        // only return user nickname and nothing else
        return $this->hasMany(User::class, 'referrer_id')->select('id', 'github_nickname', 'created_at');
    }
}
