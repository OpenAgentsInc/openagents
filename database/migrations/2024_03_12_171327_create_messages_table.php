<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('messages', function (Blueprint $table) {
            $table->id();

            // Messages must belong to one thread.
            $table->foreignId('thread_id')->constrained();

            // Messages may be sent by a user, or null if agent or unauthed
            $table->foreignId('user_id')->nullable();

            // Messages may be sent by an agent, or null if user or unauthed
            $table->foreignId('agent_id')->nullable();

            // Message content - will be changed to longtext
            $table->longText('body');

            // Model used for inference
            $table->text('model')->nullable();

            // Session ID for unauthenticated users
            $table->string('session_id')->nullable();

            $table->integer('input_tokens')->nullable();
            $table->integer('output_tokens')->nullable();

            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('messages');
    }
};
