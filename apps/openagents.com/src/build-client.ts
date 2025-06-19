/**
 * Build client-side bundles using Bun's bundler
 */
import { join } from 'path'
import { fileURLToPath } from 'url'
import { mkdir } from 'fs/promises'

const __dirname = fileURLToPath(new URL('.', import.meta.url))

async function buildClient() {
  console.log('Building client bundles...')
  
  // Ensure output directory exists
  const outdir = join(__dirname, '../public/js')
  await mkdir(outdir, { recursive: true })
  
  try {
    // Build chat client bundle
    const result = await Bun.build({
      entrypoints: [join(__dirname, 'client/chat-client.ts')],
      outdir,
      target: 'browser',
      format: 'esm',
      splitting: true,
      minify: process.env.NODE_ENV === 'production',
      naming: {
        entry: '[name].[ext]',
        chunk: '[name]-[hash].[ext]',
      },
      external: [],
    })
    
    if (!result.success) {
      console.error('Build failed:')
      for (const message of result.logs) {
        console.error(message)
      }
      process.exit(1)
    }
    
    console.log('Client bundles built successfully!')
    console.log('Output files:', result.outputs.map(o => o.path))
  } catch (error) {
    console.error('Build error:', error)
    process.exit(1)
  }
}

buildClient()