<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('runtime_shadow_diffs', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->string('thread_id', 64);
            $table->unsignedBigInteger('user_id');
            $table->string('runtime_driver', 32);
            $table->string('shadow_driver', 32)->nullable();
            $table->string('status', 32);
            $table->json('request_meta')->nullable();
            $table->json('primary_summary');
            $table->json('shadow_summary')->nullable();
            $table->json('diff');
            $table->timestamps();

            $table->index(['thread_id', 'created_at']);
            $table->index(['status', 'created_at']);
            $table->index(['user_id', 'created_at']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('runtime_shadow_diffs');
    }
};
