import * as LucideIcons from 'lucide-react';
import { createElement } from 'react';

interface IconLoaderProps {
  icon: string;
  size?: number;
  className?: string;
}

export function IconLoader({ icon, size = 24, className = '' }: IconLoaderProps) {
  // Check if it's an emoji or text
  if (icon && (icon.length <= 2 || /\p{Emoji}/u.test(icon))) {
    return <span className={className}>{icon}</span>;
  }

  // Try to load a Lucide icon
  const LucideIcon = (LucideIcons as Record<string, any>)[icon];
  if (LucideIcon) {
    return createElement(LucideIcon, { size, className });
  }

  // Fallback to default icon
  return createElement(LucideIcons.Square, { size, className });
}