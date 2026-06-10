export type ContainerFetchInit = Readonly<{
  body?: string | undefined
  method?: string | undefined
}>

export type ContainerPathFetch = (
  path: string,
  init?: ContainerFetchInit,
) => Promise<Response>
