import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export const cn = (...inputs: ReadonlyArray<ClassValue>): string =>
  twMerge(clsx(inputs))
