import { Button } from '@/components/ui/button';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useEffect, useState } from 'react';
import * as LucideIcons from 'lucide-react';

// Common project-related icons
const commonIcons = [
  '📋', '📝', '📊', '📈', '📉', '🔍', '🔧', '⚙️', '📱', '💻', '🖥️', '🌐', '🔐',
  '📁', '📂', '📚', '📄', '📑', '🔔', '🔄', '⏱️', '📆', '🗓️', '🎯', '🚀'
];

// Lucide icon names that are commonly used for projects
const lucideIconNames = [
  'Activity', 'AlertCircle', 'Archive', 'BarChart', 'Bell', 'Book', 'Briefcase',
  'Calendar', 'CheckCircle', 'Clipboard', 'Clock', 'Code', 'Cog', 'Database',
  'File', 'FileText', 'Flag', 'Folder', 'Globe', 'Home', 'Image', 'Inbox',
  'Key', 'Layout', 'Link', 'List', 'Lock', 'Mail', 'Map', 'Monitor', 'Package',
  'Paperclip', 'Pencil', 'Phone', 'PieChart', 'Settings', 'Shield', 'Star',
  'Tag', 'Target', 'Terminal', 'Tool', 'Truck', 'Users', 'Video', 'Zap'
];

interface IconPickerProps {
  icon: string;
  onChange: (icon: string) => void;
}

export function IconPicker({ icon, onChange }: IconPickerProps) {
  const [searchTerm, setSearchTerm] = useState('');
  const [filteredIcons, setFilteredIcons] = useState<(typeof lucideIconNames)[number][]>(lucideIconNames);
  const [showLucideIcons, setShowLucideIcons] = useState(false);

  // When search term changes, filter icons
  useEffect(() => {
    if (!searchTerm) {
      setFilteredIcons(lucideIconNames);
      return;
    }

    const filtered = lucideIconNames.filter(name => 
      name.toLowerCase().includes(searchTerm.toLowerCase())
    );
    setFilteredIcons(filtered);
  }, [searchTerm]);

  const isLucideIcon = (iconValue: string) => {
    return lucideIconNames.includes(iconValue as any);
  };

  const renderIcon = () => {
    if (isLucideIcon(icon)) {
      const LucideIcon = (LucideIcons as any)[icon];
      return <LucideIcon className="size-4" />;
    }
    return <span>{icon}</span>;
  };

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" className="gap-2 w-full justify-between">
          <div className="flex items-center gap-2">
            {renderIcon()}
            <span>{isLucideIcon(icon) ? icon : 'Emoji'}</span>
          </div>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[300px] p-4">
        <div className="space-y-4">
          <div className="space-y-2">
            <Label>Search Icons</Label>
            <Input
              placeholder="Search..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
          </div>

          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant={!showLucideIcons ? "default" : "outline"}
              onClick={() => setShowLucideIcons(false)}
            >
              Emoji
            </Button>
            <Button
              size="sm"
              variant={showLucideIcons ? "default" : "outline"}
              onClick={() => setShowLucideIcons(true)}
            >
              Icons
            </Button>
          </div>

          {!showLucideIcons ? (
            <div className="grid grid-cols-6 gap-2">
              {commonIcons.map((emoji) => (
                <Button
                  key={emoji}
                  variant="outline"
                  className="size-8 p-0"
                  onClick={() => onChange(emoji)}
                >
                  {emoji}
                </Button>
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-4 gap-2 max-h-[200px] overflow-y-auto">
              {filteredIcons.map((iconName) => {
                const LucideIcon = (LucideIcons as any)[iconName];
                return (
                  <Button
                    key={iconName}
                    variant="outline"
                    className="size-10 p-0"
                    onClick={() => onChange(iconName)}
                  >
                    <LucideIcon className="size-5" />
                  </Button>
                );
              })}
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}