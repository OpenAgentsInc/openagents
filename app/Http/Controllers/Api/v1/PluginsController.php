<?php

namespace App\Http\Controllers\Api\v1;

use App\Http\Controllers\Controller;
use App\Http\Resources\PluginSecretResource;
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
        $limit = intval($request->query('limit', 100));
        if ($limit > 1000) {
            $limit = 1000;
        }

        $offset = intval($request->query('offset', 0));
        $plugins = Plugin::query()
            ->where('suspended', '')
            ->where('enabled', true)
            ->oldest('created_at')
            ->skip($offset)
            ->take($limit)->get();

        $pluginViews = [];
        foreach ($plugins as $plugin) {
            $url = route('api.plugins.view', ['plugin' => $plugin->id]);
            $pluginViews[] = $url;
        }

        return response()->json($pluginViews, 200);
    }

    /**
     *  View a plugin
     */
    public function show(Request $request, Plugin $plugin)
    {

        $payment = '';
        if ($plugin->payment) {
            $payment = 'lightning:'.$plugin->payment;
        } elseif ($plugin->user->lightning_address) {
            $payment = 'lightning:'.$plugin->user->lightning_address;
        } else {
            // TODO: pay to user id?
        }

        $picture = $plugin->picture ? json_decode($plugin->picture, true) : null;
        $sockets = [
            'in' => [],
            'out' => [],
        ];

        $inputs = json_decode($plugin->input_sockets, true);
        $outputs = json_decode($plugin->output_sockets, true);

        foreach ($inputs as $input) {
            $sockets['in'][$input['name']] = [
                'name' => $input['name'],
                'type' => $input['type'],
                'description' => $input['description'],
            ];
        }

        foreach ($outputs as $output) {
            // HOTFIX
            if (! isset($output['name'])) {
                $output['name'] = $output['title'];
            }

            $sockets['out'][$output['name']] = [
                'name' => $output['name'],
                'type' => $output['type'],
                'description' => $output['description'],
            ];
        }

        $out = [
            'meta' => [
                'id' => 'oaplugin'.$plugin->id,
                'name' => $plugin->name,
                'description' => $plugin->description,
                'tos' => $plugin->tos,
                'privacy' => $plugin->privacy,
                'author' => $plugin->user->name,
                'web' => $plugin->web,
                'picture' => $picture ? $picture['url'] : '',
                'tags' => array_merge(['tool'], json_decode($plugin->tags, true)),
                'payment' => $payment,
            ],
            'mini-template' => [
                'main' => $plugin->file_link,
                'input' => $plugin->input_template,
                'allowed_hosts' => json_decode($plugin->allowed_hosts, true),
            ],
            'sockets' => $sockets,
        ];

        if ($plugin->price_msats) {
            if (! isset($out['meta']['prices'])) {
                $out['meta']['prices'] = [];
            }
            $out['meta']['prices'][] = [
                'amount' => $plugin->price_msats,
                'currency' => 'bitcoin',
                'protocol' => 'lightning',
            ];
        }

        return response()->json($out, 200);
    }

    /**
     *  Get the plugin secret
     *
     * @response  PluginSecretResource
     */
    public function secret(Request $request)
    {

        $secret = $request->query('secret');

        if (config('pool.webhook_secret') && $secret !== config('pool.webhook_secret')) {
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
