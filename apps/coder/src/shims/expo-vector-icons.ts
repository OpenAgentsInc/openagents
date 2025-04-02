// This file provides a shim for Expo vector icons in Electron
import React from 'react';
import { IoniconsMock } from './mock-ionicons';

// Export our mock implementations directly
export const Ionicons = IoniconsMock;
export const MaterialCommunityIcons = IoniconsMock; // Reuse the same mock for MaterialCommunityIcons