import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ReadonlyArray<ClassValue>) {
  return twMerge(clsx(inputs))
}
