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
    Schema::create('steps', function (Blueprint $table) {
      $table->id();
      $table->foreignIdFor(\App\Models\Agent::class)->constrained()->cascadeOnDelete();
      $table->foreignIdFor(\App\Models\Run::class)->constrained()->cascadeOnDelete();
      $table->string('description')->nullable();
      $table->string('status')->nullable();
      $table->json('input')->nullable();
      $table->json('output')->nullable();
      $table->timestamps();
    });
  }

  /**
   * Reverse the migrations.
   */
  public function down(): void
  {
    Schema::dropIfExists('steps');
  }
};
