<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    protected $fillable = [
        'kind', // TODO: remove unused
        'name',
        'description',
        'tos',
        'privacy',
        'web',
        'picture',
        'tags',
        'mini_template', // TODO: unused, remove
        'input_template', // TODO: rename to input_sockets
        'output_template',// TODO: remove unused
        'plugin_input', // TODO: rename to input_template
        'secrets',
        'file_link',
        'user_id',
        'author',
        'payment',
        'wasm_upload',
    ];

    protected $casts = [
        'tags' => 'json',
        'mini_template' => 'json',
        'secrets' => 'json',
        'wasm_upload' => 'json',

    ];

    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
