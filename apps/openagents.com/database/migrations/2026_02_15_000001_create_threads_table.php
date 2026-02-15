<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('threads', function (Blueprint $table) {
            $table->string('id', 36)->primary();
            $table->foreignId('user_id')->index();
            $table->string('title');
            $table->timestamps();

            $table->index(['user_id', 'updated_at']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('threads');
    }
};
