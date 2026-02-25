import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\AutopilotStreamController::stream
* @see app/Http/Controllers/Api/AutopilotStreamController.php:32
* @route '/api/autopilots/{autopilot}/stream'
*/
export const stream = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stream.url(args, options),
    method: 'post',
})

stream.definition = {
    methods: ["post"],
    url: '/api/autopilots/{autopilot}/stream',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AutopilotStreamController::stream
* @see app/Http/Controllers/Api/AutopilotStreamController.php:32
* @route '/api/autopilots/{autopilot}/stream'
*/
stream.url = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { autopilot: args }
    }

    if (Array.isArray(args)) {
        args = {
            autopilot: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        autopilot: args.autopilot,
    }

    return stream.definition.url
            .replace('{autopilot}', parsedArgs.autopilot.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AutopilotStreamController::stream
* @see app/Http/Controllers/Api/AutopilotStreamController.php:32
* @route '/api/autopilots/{autopilot}/stream'
*/
stream.post = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stream.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AutopilotStreamController::stream
* @see app/Http/Controllers/Api/AutopilotStreamController.php:32
* @route '/api/autopilots/{autopilot}/stream'
*/
const streamForm = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stream.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AutopilotStreamController::stream
* @see app/Http/Controllers/Api/AutopilotStreamController.php:32
* @route '/api/autopilots/{autopilot}/stream'
*/
streamForm.post = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stream.url(args, options),
    method: 'post',
})

stream.form = streamForm

const AutopilotStreamController = { stream }

export default AutopilotStreamController