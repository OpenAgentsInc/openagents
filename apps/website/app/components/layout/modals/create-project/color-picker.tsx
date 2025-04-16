import { Button } from '@/components/ui/button';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useState } from 'react';

// Common project colors
const commonColors = [
  '#6366F1', // Indigo
  '#3B82F6', // Blue
  '#10B981', // Emerald
  '#F59E0B', // Amber
  '#EF4444', // Red
  '#8B5CF6', // Violet
  '#EC4899', // Pink
  '#06B6D4', // Cyan
  '#14B8A6', // Teal
  '#F97316', // Orange
  '#84CC16', // Lime
  '#4F46E5', // Indigo (darker)
  '#3730A3', // Indigo (darkest)
  '#0369A1', // Blue (darker)
  '#0C4A6E', // Blue (darkest)
  '#047857', // Emerald (darker)
  '#064E3B', // Emerald (darkest)
  '#B45309', // Amber (darker)
  '#78350F', // Amber (darkest)
  '#B91C1C', // Red (darker)
  '#7F1D1D', // Red (darkest)
];

interface ColorPickerProps {
  color: string;
  onChange: (color: string) => void;
}

export function ColorPicker({ color, onChange }: ColorPickerProps) {
  const [customColor, setCustomColor] = useState(color);

  const handleCustomColorChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setCustomColor(e.target.value);
  };

  const handleCustomColorBlur = () => {
    onChange(customColor);
  };

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" className="gap-2 w-full justify-between">
          <div className="flex items-center gap-2">
            <div 
              className="size-4 rounded-full" 
              style={{ backgroundColor: color }}
            />
            <span>{color}</span>
          </div>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[300px] p-4">
        <div className="space-y-4">
          <div className="space-y-2">
            <Label>Custom Color</Label>
            <div className="flex items-center gap-2">
              <Input
                type="color"
                value={customColor}
                onChange={handleCustomColorChange}
                onBlur={handleCustomColorBlur}
                className="size-10 p-1 cursor-pointer"
              />
              <Input
                type="text"
                value={customColor}
                onChange={handleCustomColorChange}
                onBlur={handleCustomColorBlur}
                className="flex-1"
                placeholder="#RRGGBB"
              />
            </div>
          </div>

          <div className="space-y-2">
            <Label>Preset Colors</Label>
            <div className="grid grid-cols-7 gap-2">
              {commonColors.map((presetColor) => (
                <Button
                  key={presetColor}
                  variant="outline"
                  className="size-8 p-0"
                  style={{ backgroundColor: presetColor }}
                  onClick={() => onChange(presetColor)}
                />
              ))}
            </div>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}