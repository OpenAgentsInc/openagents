<?php

namespace App\Http\Controllers\Api\v1;

use App\Http\Controllers\Controller;
use App\Http\Resources\DocumentResource;
use App\Models\Agent;
use App\Models\AgentFile;
use Illuminate\Http\Request;

class DocumentsController extends Controller
{
    /**
     *  Get agents documents
     *
     * @response  DocumentResource
     */
    public function index(Request $request, Agent $agent)
    {

        $secret = $request->query('secret');

        if (config('nostr.webhook_secret') && $secret !== config('nostr.webhook_secret')) {
            return response()->json(['message' => 'Invalid token'], 403);
        }
        $secret = request()->query('secret');

        if (config('nostr.webhook_secret') && $secret !== config('nostr.webhook_secret')) {
            return response()->json(['error' => 'Invalid token'], 403);
        }
        $files = AgentFile::where('agent_id', $agent->id)->get();

        return DocumentResource::collection($files);
    }
}
