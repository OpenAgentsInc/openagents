import type { Project } from '@/lib/projects-store';

const norm = (s: string) => s.toLowerCase().trim();

export function pickProjectFromUtterance(utterance: string, projects: Project[]): Project | null {
  const u = norm(utterance);
  for (const p of projects) {
    const names = [p.name, ...(p.voiceAliases || [])].map(norm).filter(Boolean);
    for (const n of names) {
      if (!n) continue;
      if (u === n) return p;
      if (u.startsWith(n + ' ') || u.endsWith(' ' + n) || u.includes(' ' + n + ' ')) return p;
      if (u.includes(n) && n.length >= 4) return p; // coarse fuzzy
    }
  }
  return null;
}

