import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Button } from '@/components/ui/button';
import { useState } from 'react';

interface IconPickerProps {
  icon: string;
  onChange: (newIcon: string) => void;
}

const TEAM_ICONS = [
  '👥', '💻', '💼', '🔔', '🔥', '💎',
  '💡', '📊', '📃', '📗', '📚', '📜',
  '📢', '📱', '🛠', '🛡', '🚀', '🐝',
  '🌈', '🌍', '🌎', '🌏', '🌞', '🌟',
  '🌴', '🍁', '🍂', '🍃', '🍉', '🍊',
  '🍌', '🍒', '🍔', '🍕', '🍡', '🍩',
  '🍭', '🍷', '🍸', '🍺', '🍻', '🎉',
  '🎊', '🎎', '🎓', '🎤', '🎨', '🎭',
  '🎵', '🎸', '🎾', '🐍', '🐎', '🐕',
  '🐘', '🐟', '🐤', '🐦', '🐱', '🐶',
  '🐼', '🐾', '👀', '👊', '👋', '👍',
  '👏', '👻', '👽', '💄', '💊', '💋',
  '💪', '💭', '💰', '💸', '💾', '💿',
  '📄', '📍', '📎', '📏', '📓', '📕',
  '📖', '📘', '📝', '📟', '📠', '📡',
  '📦', '📧', '📬', '📮', '📲', '📳',
  '📴', '📷', '📹', '📺', '📻', '📼',
  '🔊', '🔍', '🔑', '🔒', '🔖', '🔥',
  '🔺', '🔽', '🕯', '🕰', '🗒', '🗞',
  '😀', '😃', '😍', '😘', '😭', '😱',
  '😶', '🚀', '🛂', '🛃', '🛌', '🤖',
];

export function IconPicker({ icon, onChange }: IconPickerProps) {
  const [open, setOpen] = useState(false);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          className="w-full justify-between"
        >
          <span className="text-xl">{icon}</span>
          <span className="text-xs text-muted-foreground ml-2">Select icon</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-full p-0" align="start">
        <div className="grid grid-cols-9 gap-1 p-2 max-h-[200px] overflow-y-auto">
          {TEAM_ICONS.map((emoji) => (
            <Button
              key={emoji}
              variant="ghost"
              className="h-9 w-9 p-0 text-lg"
              onClick={() => {
                onChange(emoji);
                setOpen(false);
              }}
            >
              {emoji}
            </Button>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
