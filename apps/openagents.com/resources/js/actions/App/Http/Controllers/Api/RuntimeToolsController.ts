import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\RuntimeToolsController::execute
* @see app/Http/Controllers/Api/RuntimeToolsController.php:12
* @route '/api/runtime/tools/execute'
*/
export const execute = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: execute.url(options),
    method: 'post',
})

execute.definition = {
    methods: ["post"],
    url: '/api/runtime/tools/execute',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeToolsController::execute
* @see app/Http/Controllers/Api/RuntimeToolsController.php:12
* @route '/api/runtime/tools/execute'
*/
execute.url = (options?: RouteQueryOptions) => {
    return execute.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeToolsController::execute
* @see app/Http/Controllers/Api/RuntimeToolsController.php:12
* @route '/api/runtime/tools/execute'
*/
execute.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: execute.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeToolsController::execute
* @see app/Http/Controllers/Api/RuntimeToolsController.php:12
* @route '/api/runtime/tools/execute'
*/
const executeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: execute.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeToolsController::execute
* @see app/Http/Controllers/Api/RuntimeToolsController.php:12
* @route '/api/runtime/tools/execute'
*/
executeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: execute.url(options),
    method: 'post',
})

execute.form = executeForm

const RuntimeToolsController = { execute }

export default RuntimeToolsController