<?php

namespace App\Http\Controllers\Api\v1;

use App\Http\Controllers\Controller;
use App\Http\Resources\PluginResource;
use App\Http\Resources\PluginSecretResource;
use App\Http\Resources\PluginsResource;
use App\Models\Plugin;
use Illuminate\Http\Request;

class PluginsController extends Controller
{
    /**
     *  List available plugins.
     *
     * @response array ["http://openagents.test/api/v1/plugins/view/8"]
     */
    public function index(Request $request)
    {

        $secret = $request->query('secret');

        if (config('nostr.webhook_secret') && $secret !== config('nostr.webhook_secret')) {
            return response()->json(['error' => 'Invalid token'], 403);
        }

        $plugins = Plugin::query()->oldest('created_at')->paginate(100);

        // Transform the paginated plugins collection using PluginsResource
        $transformedPlugins = PluginsResource::collection($plugins);

        // Get the array of transformed plugins using the toArray() method
        $pluginsArray = $transformedPlugins->toArray($request);

        // Extract the plugins URLs using the 'through' method
        $pluginsUrls = collect($pluginsArray)->pluck('0')->toArray();

        // append ?secret=XXX to the plugin urls
        for ($i = 0; $i < count($pluginsUrls); $i++) {
            $pluginsUrls[$i] = $pluginsUrls[$i].'?secret='.$secret;
        }

        return response()->json($pluginsUrls, 200);

    }

    /**
     *  View a plugin
     *
     * @response PluginResource
     */
    public function show(Request $request, Plugin $plugin)
    {
        $secret = $request->query('secret');

        if (config('nostr.webhook_secret') && $secret !== config('nostr.webhook_secret')) {
            return response()->json(['message' => 'Invalid token'], 403);
        }

        return new PluginResource($plugin);
    }

    /**
     *  Get the plugin secret
     *
     * @response  PluginSecretResource
     */
    public function secret(Request $request)
    {

        $secret = $request->query('secret');

        if (config('nostr.webhook_secret') && $secret !== config('nostr.webhook_secret')) {
            return response()->json(['message' => 'Invalid token'], 403);
        }

        $file_link = $request->query('plugin-url');
        $plugin = Plugin::where('file_link', $file_link)->first();
        if ($plugin) {
            return new PluginSecretResource($plugin);
        }

        return response()->json(['message' => 'Plugin not found'], 404);
    }
}
