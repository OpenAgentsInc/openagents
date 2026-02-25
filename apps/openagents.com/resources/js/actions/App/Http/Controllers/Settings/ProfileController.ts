import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Settings\ProfileController::edit
* @see app/Http/Controllers/Settings/ProfileController.php:22
* @route '/settings/profile'
*/
export const edit = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: edit.url(options),
    method: 'get',
})

edit.definition = {
    methods: ["get","head"],
    url: '/settings/profile',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Settings\ProfileController::edit
* @see app/Http/Controllers/Settings/ProfileController.php:22
* @route '/settings/profile'
*/
edit.url = (options?: RouteQueryOptions) => {
    return edit.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\ProfileController::edit
* @see app/Http/Controllers/Settings/ProfileController.php:22
* @route '/settings/profile'
*/
edit.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: edit.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::edit
* @see app/Http/Controllers/Settings/ProfileController.php:22
* @route '/settings/profile'
*/
edit.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: edit.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::edit
* @see app/Http/Controllers/Settings/ProfileController.php:22
* @route '/settings/profile'
*/
const editForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: edit.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::edit
* @see app/Http/Controllers/Settings/ProfileController.php:22
* @route '/settings/profile'
*/
editForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: edit.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::edit
* @see app/Http/Controllers/Settings/ProfileController.php:22
* @route '/settings/profile'
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
* @see \App\Http\Controllers\Settings\ProfileController::editAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:32
* @route '/settings/autopilot'
*/
export const editAutopilot = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: editAutopilot.url(options),
    method: 'get',
})

editAutopilot.definition = {
    methods: ["get","head"],
    url: '/settings/autopilot',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Settings\ProfileController::editAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:32
* @route '/settings/autopilot'
*/
editAutopilot.url = (options?: RouteQueryOptions) => {
    return editAutopilot.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\ProfileController::editAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:32
* @route '/settings/autopilot'
*/
editAutopilot.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: editAutopilot.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::editAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:32
* @route '/settings/autopilot'
*/
editAutopilot.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: editAutopilot.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::editAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:32
* @route '/settings/autopilot'
*/
const editAutopilotForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: editAutopilot.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::editAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:32
* @route '/settings/autopilot'
*/
editAutopilotForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: editAutopilot.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::editAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:32
* @route '/settings/autopilot'
*/
editAutopilotForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: editAutopilot.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

editAutopilot.form = editAutopilotForm

/**
* @see \App\Http\Controllers\Settings\ProfileController::update
* @see app/Http/Controllers/Settings/ProfileController.php:43
* @route '/settings/profile'
*/
export const update = (options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: update.url(options),
    method: 'patch',
})

update.definition = {
    methods: ["patch"],
    url: '/settings/profile',
} satisfies RouteDefinition<["patch"]>

/**
* @see \App\Http\Controllers\Settings\ProfileController::update
* @see app/Http/Controllers/Settings/ProfileController.php:43
* @route '/settings/profile'
*/
update.url = (options?: RouteQueryOptions) => {
    return update.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\ProfileController::update
* @see app/Http/Controllers/Settings/ProfileController.php:43
* @route '/settings/profile'
*/
update.patch = (options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: update.url(options),
    method: 'patch',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::update
* @see app/Http/Controllers/Settings/ProfileController.php:43
* @route '/settings/profile'
*/
const updateForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: update.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::update
* @see app/Http/Controllers/Settings/ProfileController.php:43
* @route '/settings/profile'
*/
updateForm.patch = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: update.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

update.form = updateForm

/**
* @see \App\Http\Controllers\Settings\ProfileController::updateAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:58
* @route '/settings/autopilot'
*/
export const updateAutopilot = (options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: updateAutopilot.url(options),
    method: 'patch',
})

updateAutopilot.definition = {
    methods: ["patch"],
    url: '/settings/autopilot',
} satisfies RouteDefinition<["patch"]>

/**
* @see \App\Http\Controllers\Settings\ProfileController::updateAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:58
* @route '/settings/autopilot'
*/
updateAutopilot.url = (options?: RouteQueryOptions) => {
    return updateAutopilot.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\ProfileController::updateAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:58
* @route '/settings/autopilot'
*/
updateAutopilot.patch = (options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: updateAutopilot.url(options),
    method: 'patch',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::updateAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:58
* @route '/settings/autopilot'
*/
const updateAutopilotForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: updateAutopilot.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::updateAutopilot
* @see app/Http/Controllers/Settings/ProfileController.php:58
* @route '/settings/autopilot'
*/
updateAutopilotForm.patch = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: updateAutopilot.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

updateAutopilot.form = updateAutopilotForm

/**
* @see \App\Http\Controllers\Settings\ProfileController::destroy
* @see app/Http/Controllers/Settings/ProfileController.php:119
* @route '/settings/profile'
*/
export const destroy = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroy.url(options),
    method: 'delete',
})

destroy.definition = {
    methods: ["delete"],
    url: '/settings/profile',
} satisfies RouteDefinition<["delete"]>

/**
* @see \App\Http\Controllers\Settings\ProfileController::destroy
* @see app/Http/Controllers/Settings/ProfileController.php:119
* @route '/settings/profile'
*/
destroy.url = (options?: RouteQueryOptions) => {
    return destroy.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Settings\ProfileController::destroy
* @see app/Http/Controllers/Settings/ProfileController.php:119
* @route '/settings/profile'
*/
destroy.delete = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroy.url(options),
    method: 'delete',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::destroy
* @see app/Http/Controllers/Settings/ProfileController.php:119
* @route '/settings/profile'
*/
const destroyForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroy.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Settings\ProfileController::destroy
* @see app/Http/Controllers/Settings/ProfileController.php:119
* @route '/settings/profile'
*/
destroyForm.delete = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroy.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

destroy.form = destroyForm

const ProfileController = { edit, editAutopilot, update, updateAutopilot, destroy }

export default ProfileController