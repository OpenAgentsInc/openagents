import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"

type GeneratorKind = "api-core" | "component" | "screen"

type RenderContext = Readonly<{
  camelName: string
  kebabName: string
  name: string
  pascalName: string
}>

type TemplatePlan = Readonly<{
  destination: (context: RenderContext) => string
  template: string
}>

const mobileRoot = join(import.meta.dir, "..")

const plans: Record<GeneratorKind, readonly TemplatePlan[]> = {
  "api-core": [
    {
      destination: context => `src/api/${context.kebabName}-core.ts`,
      template: "api-core/NAME-core.ts.ejs",
    },
    {
      destination: context => `tests/${context.kebabName}-api-core.test.ts`,
      template: "api-core-test/NAME-api-core.test.ts.ejs",
    },
  ],
  component: [
    {
      destination: context => `src/components/${context.kebabName}.tsx`,
      template: "component/NAME.tsx.ejs",
    },
    {
      destination: context => `tests/${context.kebabName}.test.tsx`,
      template: "component-test/NAME.test.tsx.ejs",
    },
    {
      destination: context => `src/components/${context.kebabName}.stories.tsx`,
      template: "component-stories/NAME.stories.tsx.ejs",
    },
  ],
  screen: [
    {
      destination: context => `src/screens/${context.kebabName}-screen.tsx`,
      template: "screen/NAME-screen.tsx.ejs",
    },
    {
      destination: context => `tests/${context.kebabName}-screen.test.tsx`,
      template: "screen-mount-test/NAME-screen.test.tsx.ejs",
    },
    {
      destination: context => `src/screens/${context.kebabName}-screen.stories.tsx`,
      template: "screen-stories/NAME-screen.stories.tsx.ejs",
    },
    {
      destination: context => `tests/${context.kebabName}-contract.test.ts`,
      template: "screen-contract/NAME-contract.test.ts.ejs",
    },
    {
      destination: context => `.maestro/generated/${context.kebabName}-screen.yaml`,
      template: "screen-maestro/NAME-screen.yaml.ejs",
    },
    {
      destination: context => `src/qa/visual-baselines/${context.kebabName}-screen.ts`,
      template: "screen-visual/NAME-screen.ts.ejs",
    },
  ],
}

const [, , rawKind, rawName] = process.argv

if (!isGeneratorKind(rawKind) || rawName === undefined) {
  fail("Usage: bun run scripts/generate.ts <screen|component|api-core> <Name>")
}

const context = namesFor(rawName)
const outRoot = process.env.KHALA_MOBILE_GENERATE_OUT_DIR ?? mobileRoot

for (const plan of plans[rawKind]) {
  const templatePath = join(mobileRoot, "templates", plan.template)
  const destinationPath = join(outRoot, plan.destination(context))
  const rendered = renderTemplate(readFileSync(templatePath, "utf8"), context)

  if (existsSync(destinationPath) && process.env.KHALA_MOBILE_GENERATE_FORCE !== "1") {
    fail(`Refusing to overwrite ${destinationPath}. Set KHALA_MOBILE_GENERATE_FORCE=1 to replace it.`)
  }

  mkdirSync(dirname(destinationPath), { recursive: true })
  writeFileSync(destinationPath, rendered)
  process.stdout.write(`${destinationPath}\n`)
}

function isGeneratorKind(value: string | undefined): value is GeneratorKind {
  return value === "screen" || value === "component" || value === "api-core"
}

function namesFor(input: string): RenderContext {
  const words = input
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)

  if (words.length === 0) fail("Name must include at least one alphanumeric word.")

  const pascalName = words.map(capitalize).join("")
  const camelName = `${pascalName.charAt(0).toLowerCase()}${pascalName.slice(1)}`
  const kebabName = words.map(word => word.toLowerCase()).join("-")

  return {
    camelName,
    kebabName,
    name: input,
    pascalName,
  }
}

function renderTemplate(template: string, context: RenderContext): string {
  return template
    .replaceAll("<%= camelName %>", context.camelName)
    .replaceAll("<%= kebabName %>", context.kebabName)
    .replaceAll("<%= name %>", context.name)
    .replaceAll("<%= pascalName %>", context.pascalName)
    .replaceAll("NAME", context.pascalName)
}

function capitalize(word: string): string {
  return `${word.charAt(0).toUpperCase()}${word.slice(1).toLowerCase()}`
}

function fail(message: string): never {
  throw new Error(message)
}
