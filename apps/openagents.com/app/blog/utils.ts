import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import readingTime from 'reading-time';

export interface BlogPost {
  slug: string;
  title: string;
  date: string;
  summary?: string;
  image?: string;
  content: string;
  readingTime: string;
}

// Get all MDX files from the blog directory - hardcoded for now
export function getAllPosts(): BlogPost[] {
  // Hardcode the blog posts for now since we're using individual pages
  const posts = [
    {
      slug: 'agent-payments-api',
      title: 'Introducing the Agent Payments API',
      date: '2025-05-05',
      summary: 'We built the easiest way for AI agents to send and receive money. Open beta is now available globally for all developers.',
      image: '/images/blog02.png',
      content: '',
      readingTime: '5 min read'
    },
    {
      slug: 'ai-agents-at-off-2025',
      title: 'AI Agents @ OFF 2025',
      date: '2025-06-02',
      summary: 'We gave an introductory talk about AI agents at the Oslo Freedom Forum.',
      image: '/images/blog06.png',
      content: '',
      readingTime: '10 min read'
    },
    {
      slug: 'intro-to-ai-coding-agents',
      title: 'Intro to AI Coding Agents',
      date: '2025-04-30',
      summary: 'Our PlebLab workshop has equal parts education and shit talking.',
      image: '/images/blog01.png',
      content: '',
      readingTime: '8 min read'
    },
    {
      slug: 'discord',
      title: "We're on Discord",
      date: '2025-05-10',
      summary: 'OpenAgents now has a Discord. You should join.',
      image: '/images/blog03.png',
      content: '',
      readingTime: '2 min read'
    },
    {
      slug: 'gputopia',
      title: 'GPUtopia 2.0',
      date: '2025-05-14',
      summary: "We're rebooting our swarm compute network as OpenAgents Compute.",
      image: '/images/blog05.png',
      content: '',
      readingTime: '4 min read'
    },
    {
      slug: 'openagents-wallet',
      title: 'Introducing the OpenAgents Wallet',
      date: '2025-05-13',
      summary: 'Earning and spending bitcoin with AI agents should be easy and fun. Now it is!',
      image: '/images/blog04.png',
      content: '',
      readingTime: '3 min read'
    },
    {
      slug: 'outage-lessons',
      title: 'Analyzing the June 12 Internet Outage',
      date: '2025-06-13',
      summary: 'Claude\'s after-action report from yesterday\'s "seven-hour digital nightmare"',
      image: '/images/blog07.png',
      content: '',
      readingTime: '12 min read'
    }
  ];
  
  // Sort posts by date (newest first)
  return posts.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
}

// Get a single post by slug
export function getPostBySlug(slug: string): BlogPost | null {
  const postsDirectory = path.join(process.cwd(), 'app', 'blog', 'posts');
  const fullPath = path.join(postsDirectory, `${slug}.mdx`);
  
  if (!fs.existsSync(fullPath)) {
    return null;
  }
  
  const fileContents = fs.readFileSync(fullPath, 'utf8');
  const { data, content } = matter(fileContents);
  const stats = readingTime(content);
  
  return {
    slug,
    title: data.title || slug,
    date: data.date || new Date().toISOString(),
    summary: data.summary,
    image: data.image,
    content,
    readingTime: stats.text
  };
}