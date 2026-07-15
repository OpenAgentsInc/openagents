/** Static Astro public pages mounted by the Cloud Run monolith. */
import { existsSync } from 'node:fs'
import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'

const DEFAULT_ASTRO_UI_DIR = path.resolve(
  process.env['ASTRO_UI_DIR'] ??
    path.resolve(
      import.meta.dirname,
      '..',
      '..',
      '..',
      '..',
      'apps/astro/dist',
    ),
)

const contentType = (filePath: string): string => {
  switch (path.extname(filePath).toLowerCase()) {
    case '.css':
      return 'text/css; charset=utf-8'
    case '.html':
      return 'text/html; charset=utf-8'
    case '.js':
    case '.mjs':
      return 'text/javascript; charset=utf-8'
    case '.json':
      return 'application/json'
    case '.png':
      return 'image/png'
    case '.svg':
      return 'image/svg+xml'
    case '.webp':
      return 'image/webp'
    case '.woff':
      return 'font/woff'
    case '.woff2':
      return 'font/woff2'
    default:
      return 'application/octet-stream'
  }
}

const astroFileForPath = (
  pathname: string,
  astroUiDir: string,
): string | undefined => {
  if (pathname === '/astro' || pathname === '/astro/') {
    return path.join(astroUiDir, 'index.html')
  }
  if (pathname === '/install' || pathname === '/install/') {
    return path.join(astroUiDir, 'install', 'index.html')
  }
  if (!pathname.startsWith('/astro/')) {
    return undefined
  }

  let relativePath: string
  try {
    relativePath = decodeURIComponent(pathname.slice('/astro/'.length))
  } catch {
    return undefined
  }
  if (relativePath === '' || relativePath.includes('\\')) {
    return undefined
  }

  const resolved = path.resolve(astroUiDir, relativePath)
  return resolved.startsWith(`${path.resolve(astroUiDir)}${path.sep}`)
    ? resolved
    : undefined
}

export const assertAstroUiArtifactsExist = (
  astroUiDir: string = DEFAULT_ASTRO_UI_DIR,
): void => {
  if (!existsSync(path.join(astroUiDir, 'index.html'))) {
    throw new Error(
      `Astro UI artifacts missing (${astroUiDir}). Run \`pnpm run build:astro\` first.`,
    )
  }
}

export const handleAstroUiRequest = async (
  request: Request,
  astroUiDir: string = DEFAULT_ASTRO_UI_DIR,
): Promise<Response | undefined> => {
  const url = new URL(request.url)
  if (
    url.pathname !== '/astro' &&
    !url.pathname.startsWith('/astro/') &&
    url.pathname !== '/install' &&
    url.pathname !== '/install/'
  ) {
    return undefined
  }
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return Response.json(
      { error: 'method_not_allowed' },
      { status: 405, headers: { allow: 'GET, HEAD' } },
    )
  }

  const filePath = astroFileForPath(url.pathname, astroUiDir)
  if (filePath === undefined) {
    return Response.json({ error: 'not_found' }, { status: 404 })
  }
  try {
    const info = await stat(filePath)
    if (!info.isFile()) {
      return Response.json({ error: 'not_found' }, { status: 404 })
    }
    const immutable = url.pathname.startsWith('/astro/_astro/')
    return new Response(
      request.method === 'HEAD' ? null : await readFile(filePath),
      {
        headers: {
          'cache-control': immutable
            ? 'public, max-age=31536000, immutable'
            : 'public, max-age=60',
          'content-length': String(info.size),
          'content-type': contentType(filePath),
        },
      },
    )
  } catch {
    return Response.json({ error: 'not_found' }, { status: 404 })
  }
}
