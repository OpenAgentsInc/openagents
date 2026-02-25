import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../wayfinder'
/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsert
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
export const upsert = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsert.url(options),
    method: 'post',
})

upsert.definition = {
    methods: ["post"],
    url: '/settings/integrations/resend',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsert
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
upsert.url = (options?: RouteQueryOptions) => {
    return upsert.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsert
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
upsert.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsert.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsert
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
const upsertForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsert.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsert
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
upsertForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsert.url(options),
    method: 'post',
})

upsert.form = upsertForm

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnect
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
export const disconnect = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: disconnect.url(options),
    method: 'delete',
})

disconnect.definition = {
    methods: ["delete"],
    url: '/settings/integrations/resend',
} satisfies RouteDefinition<["delete"]>

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnect
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
disconnect.url = (options?: RouteQueryOptions) => {
    return disconnect.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnect
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
disconnect.delete = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: disconnect.url(options),
    method: 'delete',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnect
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
const disconnectForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: disconnect.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnect
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
disconnectForm.delete = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: disconnect.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

disconnect.form = disconnectForm

/**
* @see \App\Http\Controllers\Settings\IntegrationController::test
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
export const test = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: test.url(options),
    method: 'post',
})

test.definition = {
    methods: ["post"],
    url: '/settings/integrations/resend/test',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Settings\IntegrationController::test
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
test.url = (options?: RouteQueryOptions) => {
    return test.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\IntegrationController::test
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
test.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: test.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::test
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
const testForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: test.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::test
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
testForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: test.url(options),
    method: 'post',
})

test.form = testForm

const resend = {
    upsert: Object.assign(upsert, upsert),
    disconnect: Object.assign(disconnect, disconnect),
    test: Object.assign(test, test),
}

export default resend