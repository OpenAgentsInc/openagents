<?php

namespace App\Http\Controllers\Api\v1;

use App\Models\Agent;
use App\Models\AgentFile;
use Illuminate\Http\Request;
use App\Http\Controllers\Controller;
use App\Http\Resources\DocumentResource;

class DocumentsController extends Controller
{
    public function index(Agent $agent){
        $files = AgentFile::where('agent_id',$agent->id)->get();
        return DocumentResource::collection($files);
    }
}
