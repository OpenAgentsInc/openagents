import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../wayfinder'
import resend from './resend'
/**
* @see \App\Http\Controllers\Settings\IntegrationController::edit
* @see app/Http/Controllers/Settings/IntegrationController.php:18
* @route '/settings/integrations'
*/
export const edit = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: edit.url(options),
    method: 'get',
})

edit.definition = {
    methods: ["get","head"],
    url: '/settings/integrations',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Settings\IntegrationController::edit
* @see app/Http/Controllers/Settings/IntegrationController.php:18
* @route '/settings/integrations'
*/
edit.url = (options?: RouteQueryOptions) => {
    return edit.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\IntegrationController::edit
* @see app/Http/Controllers/Settings/IntegrationController.php:18
* @route '/settings/integrations'
*/
edit.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: edit.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::edit
* @see app/Http/Controllers/Settings/IntegrationController.php:18
* @route '/settings/integrations'
*/
edit.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: edit.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::edit
* @see app/Http/Controllers/Settings/IntegrationController.php:18
* @route '/settings/integrations'
*/
const editForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: edit.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::edit
* @see app/Http/Controllers/Settings/IntegrationController.php:18
* @route '/settings/integrations'
*/
editForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: edit.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::edit
* @see app/Http/Controllers/Settings/IntegrationController.php:18
* @route '/settings/integrations'
*/
editForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: edit.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

edit.form = editForm

const integrations = {
    edit: Object.assign(edit, edit),
    resend: Object.assign(resend, resend),
}

export default integrations