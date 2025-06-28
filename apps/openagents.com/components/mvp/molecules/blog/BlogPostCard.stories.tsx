import type { Meta, StoryObj } from '@storybook/react';
import { BlogPostCard } from './BlogPostCard';

const meta = {
  title: 'MVP/Molecules/Blog/BlogPostCard',
  component: BlogPostCard,
  parameters: {
    layout: 'centered',
    backgrounds: {
      default: 'dark',
    },
  },
  tags: ['autodocs'],
  argTypes: {
    slug: {
      control: 'text',
      description: 'URL slug for the blog post',
    },
    title: {
      control: 'text',
      description: 'Blog post title',
    },
    date: {
      control: 'date',
      description: 'Publication date',
    },
    summary: {
      control: 'text',
      description: 'Short description of the blog post',
    },
    image: {
      control: 'text',
      description: 'Featured image URL',
    },
  },
} satisfies Meta<typeof BlogPostCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    slug: 'intro-to-ai-coding-agents',
    title: 'Intro to AI Coding Agents',
    date: '2025-04-30',
    summary: 'Our PlebLab workshop has equal parts education and shit talking.',
    image: '/images/blog01.png',
  },
};

export const WithoutImage: Story = {
  args: {
    slug: 'building-with-effect',
    title: 'Building with Effect: A Functional Approach',
    date: '2025-05-15',
    summary: 'Learn how to build robust applications using Effect, a powerful TypeScript library for functional programming.',
  },
};

export const LongTitle: Story = {
  args: {
    slug: 'comprehensive-guide-to-building-ai-agents',
    title: 'A Comprehensive Guide to Building AI Agents with Bitcoin Lightning Network Integration',
    date: '2025-06-01',
    summary: 'Everything you need to know about creating autonomous agents that can send and receive Bitcoin payments.',
    image: '/images/blog02.png',
  },
};

export const MinimalContent: Story = {
  args: {
    slug: 'quick-update',
    title: 'Quick Update',
    date: '2025-06-10',
  },
};