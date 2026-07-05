import { AGENT_TOKEN_PREFIX } from '../agent-registration'

export const readBearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  return scheme?.toLowerCase() === 'bearer' && token !== undefined
    ? token
    : undefined
}

export const readPrefixedBearerToken = (
  request: Request,
  prefix: string,
): string | undefined => {
  const token = readBearerToken(request)

  return token !== undefined && token.startsWith(prefix) ? token : undefined
}

export const readAgentBearerToken = (request: Request): string | undefined =>
  readPrefixedBearerToken(request, AGENT_TOKEN_PREFIX)
