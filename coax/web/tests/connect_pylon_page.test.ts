import {test,expect} from 'bun:test'
import {readFileSync} from 'node:fs'
import {resolve} from 'node:path'
const root=resolve(import.meta.dir,'../../..'),source=readFileSync(resolve(root,'docs/connect-a-pylon.md'),'utf8')
test('canonical Markdown, copied Coax content and agent twin are byte identical',()=>{
 const constants=readFileSync(resolve(root,'apps/openagents-web/connect-pylon-content.coax'),'utf8')
 const match=constants.match(/^CP_MARKDOWN :: (".*");$/m);expect(match).not.toBeNull();expect(JSON.parse(match![1])).toBe(source)
 expect(readFileSync(resolve(root,'apps/openagents-web/docs/connect-a-pylon/index.md'),'utf8')).toBe(source)
 expect(Buffer.byteLength(source)).toBeLessThanOrEqual(16384)
})
test('all derived HTML/shader/Coax outputs pass their drift check',()=>{
 const run=Bun.spawnSync(['bun','coax/web/connect-pylon-page.ts','--check'],{cwd:root});expect(new TextDecoder().decode(run.stderr)).toBe('');expect(run.exitCode).toBe(0)
})
test('human page exposes Markdown without any visible DOM controls',()=>{
 const html=readFileSync(resolve(root,'apps/openagents-web/docs/connect-a-pylon/index.html'),'utf8')
 expect(html).toContain('type="text/markdown" href="/docs/connect-a-pylon/index.md"')
 expect(html).toContain('data-app="connect-pylon"');expect(html).toContain('.gpu main{')
 expect(html).not.toMatch(/<(button|input|form|div|pre)\b/)
 expect(html).toContain('Sign in to OpenAgents');expect(html).toContain('NIP-98')
})
test('the documented hosted decision is executable protocol JSON',()=>{
 const blocks=[...source.matchAll(/```json\n([\s\S]*?)\n```/g)].map(x=>JSON.parse(x[1]));expect(blocks).toHaveLength(1)
 const request=blocks[0];expect(request.model).toBe('elephant-1');expect(request.questions.wet.type).toBe('choice');expect(Object.keys(request.questions.wet.criteria)).toEqual(['yes','no']);expect(typeof request.state).toBe('string')
})
test('quickstart does not suggest a paid credential, inbound port or unshipped models',()=>{
 expect(source).toContain('No inbound port or public IP is required')
 expect(source).toContain('You do not need Bun, a wallet, an API credential or a paid account')
 expect(source).toContain('It does not download or pretend to serve a larger model')
 expect(source).not.toMatch(/ANTHROPIC_API_KEY|OPENAI_API_KEY|Bearer /)
})
