// Template types and configuration - Clean version for testing
export interface ProjectTemplate {
  id: string
  name: string
  description: string
  framework: 'react' | 'next' | 'vue' | 'express' | 'html' | 'python'
  category: 'frontend' | 'backend' | 'fullstack' | 'static'
  difficulty: 'beginner' | 'intermediate' | 'advanced'
  features: string[]
  tags: string[]
}

export const DEMO_TEMPLATES: ProjectTemplate[] = [
  {
    id: 'react-todo-auth',
    name: 'React Todo App with Authentication',
    description: 'A complete todo application built with React, featuring user authentication and clean interface.',
    framework: 'react',
    category: 'frontend',
    difficulty: 'intermediate',
    features: [
      'User authentication with local storage',
      'CRUD operations for todos',
      'Filter and search functionality',
      'Responsive design with Tailwind CSS'
    ],
    tags: ['React', 'TypeScript', 'Tailwind CSS', 'Authentication']
  },
  {
    id: 'nextjs-landing-page',
    name: 'Next.js Landing Page with Forms',
    description: 'A beautiful, conversion-optimized landing page built with Next.js.',
    framework: 'next',
    category: 'frontend',
    difficulty: 'intermediate',
    features: [
      'Server-side rendering with Next.js',
      'Contact form with validation',
      'Newsletter signup integration',
      'SEO optimized with metadata'
    ],
    tags: ['Next.js', 'TypeScript', 'Tailwind CSS', 'Forms']
  },
  {
    id: 'html-portfolio-site',
    name: 'HTML/CSS Portfolio Site',
    description: 'A clean, professional portfolio website built with vanilla HTML, CSS, and JavaScript.',
    framework: 'html',
    category: 'static',
    difficulty: 'beginner',
    features: [
      'Vanilla HTML, CSS, and JavaScript',
      'Smooth scroll animations',
      'Responsive grid layout',
      'Contact form with validation'
    ],
    tags: ['HTML', 'CSS', 'JavaScript', 'Portfolio']
  }
]

export function getTemplateById(id: string): ProjectTemplate | undefined {
  return DEMO_TEMPLATES.find(template => template.id === id)
}

export function deployTemplate(templateId: string, projectName: string): Promise<{ deploymentUrl: string }> {
  return new Promise(resolve => {
    setTimeout(() => {
      resolve({
        deploymentUrl: `https://${projectName.toLowerCase().replace(/\s+/g, '-')}.openagents.dev`
      })
    }, 3000)
  })
}