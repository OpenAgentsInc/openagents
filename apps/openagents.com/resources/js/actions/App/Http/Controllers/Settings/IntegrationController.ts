import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
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

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsertResend
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
export const upsertResend = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsertResend.url(options),
    method: 'post',
})

upsertResend.definition = {
    methods: ["post"],
    url: '/settings/integrations/resend',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsertResend
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
upsertResend.url = (options?: RouteQueryOptions) => {
    return upsertResend.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsertResend
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
upsertResend.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsertResend.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsertResend
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
const upsertResendForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsertResend.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::upsertResend
* @see app/Http/Controllers/Settings/IntegrationController.php:59
* @route '/settings/integrations/resend'
*/
upsertResendForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsertResend.url(options),
    method: 'post',
})

upsertResend.form = upsertResendForm

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnectResend
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
export const disconnectResend = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: disconnectResend.url(options),
    method: 'delete',
})

disconnectResend.definition = {
    methods: ["delete"],
    url: '/settings/integrations/resend',
} satisfies RouteDefinition<["delete"]>

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnectResend
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
disconnectResend.url = (options?: RouteQueryOptions) => {
    return disconnectResend.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnectResend
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
disconnectResend.delete = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: disconnectResend.url(options),
    method: 'delete',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnectResend
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
const disconnectResendForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: disconnectResend.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::disconnectResend
* @see app/Http/Controllers/Settings/IntegrationController.php:89
* @route '/settings/integrations/resend'
*/
disconnectResendForm.delete = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: disconnectResend.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

disconnectResend.form = disconnectResendForm

/**
* @see \App\Http\Controllers\Settings\IntegrationController::testResend
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
export const testResend = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: testResend.url(options),
    method: 'post',
})

testResend.definition = {
    methods: ["post"],
    url: '/settings/integrations/resend/test',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Settings\IntegrationController::testResend
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
testResend.url = (options?: RouteQueryOptions) => {
    return testResend.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\IntegrationController::testResend
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
testResend.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: testResend.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::testResend
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
const testResendForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: testResend.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\IntegrationController::testResend
* @see app/Http/Controllers/Settings/IntegrationController.php:102
* @route '/settings/integrations/resend/test'
*/
testResendForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: testResend.url(options),
    method: 'post',
})

testResend.form = testResendForm

const IntegrationController = { edit, upsertResend, disconnectResend, testResend }

export default IntegrationController