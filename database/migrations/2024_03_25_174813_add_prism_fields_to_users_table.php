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
        Schema::table('users', function (Blueprint $table) {
            $table->uuid('prism_user_id')->nullable()->after('id');
            $table->unsignedBigInteger('prism_created_at')->nullable()->after('profile_photo_path');
            $table->unsignedBigInteger('prism_updated_at')->nullable()->after('prism_created_at');
            $table->string('ln_address')->nullable()->after('prism_updated_at');
            $table->string('nwc_id')->nullable()->after('ln_address');
            $table->string('nwc_connector_type')->nullable()->after('nwc_id');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->dropColumn('prism_user_id');
            $table->dropColumn('prism_created_at');
            $table->dropColumn('prism_updated_at');
            $table->dropColumn('ln_address');
            $table->dropColumn('nwc_id');
            $table->dropColumn('nwc_connector_type');
        });
    }
};
