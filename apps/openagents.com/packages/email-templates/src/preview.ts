import { renderEmailTemplatePreviewCatalog } from './index'

const appOrigin = 'https://openagents.com'

for (const template of renderEmailTemplatePreviewCatalog(appOrigin)) {
  console.log(`${template.templateSlug}: ${template.subject}`)
}
