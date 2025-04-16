import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Button } from '@/components/ui/button';
import { useState } from 'react';

interface ColorPickerProps {
  color: string;
  onChange: (newColor: string) => void;
}

const TEAM_COLORS = [
  '#6366F1', '#8B5CF6', '#EC4899', '#F43F5E', '#EF4444', '#F97316',
  '#F59E0B', '#EAB308', '#84CC16', '#22C55E', '#10B981', '#14B8A6',
  '#06B6D4', '#0EA5E9', '#3B82F6', '#A855F7', '#D946EF', '#E879F9', 
  '#C026D3', '#4C1D95', '#1E40AF', '#0D9488', '#4D7C0F', '#B45309', 
  '#92400E', '#9A3412', '#B91C1C', '#BE123C', '#BE185D', '#A21CAF', 
  '#6B21A8', '#4338CA', '#1E3A8A', '#0C4A6E', '#065F46', '#365314', 
  '#422006', '#27272A', '#18181B', '#1C1917'
];

export function ColorPicker({ color, onChange }: ColorPickerProps) {
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
          <div className="flex items-center gap-2">
            <div className="size-5 rounded-full border" style={{ backgroundColor: color }} />
            <span className="text-xs">{color}</span>
          </div>
          <span className="text-xs text-muted-foreground">Select color</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-full p-0" align="start">
        <div className="grid grid-cols-9 gap-1 p-2 max-h-[200px] overflow-y-auto">
          {TEAM_COLORS.map((colorHex, index) => (
            <Button
              key={`${colorHex}-${index}`}
              variant="ghost"
              className="h-9 w-9 p-0 flex items-center justify-center"
              onClick={() => {
                onChange(colorHex);
                setOpen(false);
              }}
              title={colorHex}
            >
              <div
                className="size-6 rounded-full border"
                style={{ backgroundColor: colorHex }}
              />
            </Button>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}