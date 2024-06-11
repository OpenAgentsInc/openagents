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
        Schema::table('plugins', function (Blueprint $table) {
            $table->text('pending_revision_temp')->nullable();
        });

        DB::table('plugins')->get()->each(function ($plugin) {
            DB::table('plugins')
                ->where('id', $plugin->id)
                ->update(['pending_revision_temp' => json_encode($plugin->pending_revision)]);
        });

        Schema::table('plugins', function (Blueprint $table) {
            $table->dropColumn('pending_revision');
        });

        Schema::table('plugins', function (Blueprint $table) {
            $table->renameColumn('pending_revision_temp', 'pending_revision');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('plugins', function (Blueprint $table) {
            $table->json('pending_revision_temp')->nullable();
        });

        DB::table('plugins')->get()->each(function ($plugin) {
            DB::table('plugins')
                ->where('id', $plugin->id)
                ->update(['pending_revision_temp' => json_decode($plugin->pending_revision)]);
        });

        Schema::table('plugins', function (Blueprint $table) {
            $table->dropColumn('pending_revision');
        });

        Schema::table('plugins', function (Blueprint $table) {
            $table->renameColumn('pending_revision_temp', 'pending_revision');
        });
    }
};
