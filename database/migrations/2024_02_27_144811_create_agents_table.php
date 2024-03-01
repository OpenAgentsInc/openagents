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
        Schema::create('agents', function (Blueprint $table) {
            $table->id();

            // All agents belong to a user.
            $table->foreignId('user_id')->constrained()->cascadeOnDelete()->cascadeOnUpdate();

            // Agents have a user-facing name and description.
            $table->string('name');
            $table->string('description');

            // Agents have instructions.
            $table->string('instructions');

            // Agents optionally have a welcome message they'll send to users on new chats.
            $table->string('welcome_message')->nullable();

            // Agents have a bitcoin balance denominated in satoshis.
            $table->integer('balance')->default(0); // sats

            // Agents may be published (public) or not.
            $table->timestamp('published_at')->nullable();

            $table->timestamps();
            $table->softDeletes();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('agents');
    }
};
