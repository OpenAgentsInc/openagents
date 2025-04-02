// A simple React component to mock Ionicons in case the main library fails
import React from 'react';

// TypeScript interface for our component props - explicitly allows any string as icon name
// Map of icon names to simple SVG paths
const iconPaths: Record<string, string> = {
  // Ionicons
  'heart': 'M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z',
  'settings-outline': 'M19.14,12.94c0.04-0.3,0.06-0.61,0.06-0.94c0-0.32-0.02-0.64-0.07-0.94l2.03-1.58c0.18-0.14,0.23-0.41,0.12-0.61 l-1.92-3.32c-0.12-0.22-0.37-0.29-0.59-0.22l-2.39,0.96c-0.5-0.38-1.03-0.7-1.62-0.94L14.4,2.81c-0.04-0.24-0.24-0.41-0.48-0.41 h-3.84c-0.24,0-0.43,0.17-0.47,0.41L9.25,5.35C8.66,5.59,8.12,5.92,7.63,6.29L5.24,5.33c-0.22-0.08-0.47,0-0.59,0.22L2.74,8.87 C2.62,9.08,2.66,9.34,2.86,9.48l2.03,1.58C4.84,11.36,4.8,11.69,4.8,12s0.02,0.64,0.07,0.94l-2.03,1.58 c-0.18,0.14-0.23,0.41-0.12,0.61l1.92,3.32c0.12,0.22,0.37,0.29,0.59,0.22l2.39-0.96c0.5,0.38,1.03,0.7,1.62,0.94l0.36,2.54 c0.05,0.24,0.24,0.41,0.48,0.41h3.84c0.24,0,0.44-0.17,0.47-0.41l0.36-2.54c0.59-0.24,1.13-0.56,1.62-0.94l2.39,0.96 c0.22,0.08,0.47,0,0.59-0.22l1.92-3.32c0.12-0.22,0.07-0.47-0.12-0.61L19.14,12.94z M12,15.6c-1.98,0-3.6-1.62-3.6-3.6 s1.62-3.6,3.6-3.6s3.6,1.62,3.6,3.6S13.98,15.6,12,15.6z',
  'close': 'M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z',
  'chatbubble-outline': 'M12 22c5.5 0 10-4.5 10-10S17.5 2 12 2 2 6.5 2 12c0 1.9.5 3.6 1.4 5.1L2 22l4.9-1.4c1.5.9 3.2 1.4 5.1 1.4zm0-18c4.4 0 8 3.6 8 8s-3.6 8-8 8c-1.6 0-3.1-.5-4.3-1.3l-.8-.5-.8.3-2.1.6.6-2.1.3-.8-.5-.8c-.8-1.2-1.3-2.7-1.3-4.3-.1-4.5 3.5-8.1 7.9-8.1z',
  'code-slash-outline': 'M11.13 19.41l-6.98-6.98c-.3-.3-.3-.77 0-1.06l6.98-6.98c.3-.3.77-.3 1.06 0 .3.3.3.77 0 1.06L5.54 12l6.65 6.65c.3.3.3.77 0 1.06-.15.15-.34.22-.53.22s-.38-.07-.53-.22zM12.87 19.41c-.15.15-.34.22-.53.22s-.38-.07-.53-.22c-.3-.3-.3-.77 0-1.06L18.46 12l-6.65-6.65c-.3-.3-.3-.77 0-1.06.3-.3.77-.3 1.06 0l6.98 6.98c.3.3.3.77 0 1.06l-6.98 6.98z',
  'wallet-outline': 'M19.97 6.43L16.54 3H7.5C6.12 3 5 4.12 5 5.5v13C5 19.88 6.12 21 7.5 21h9c1.38 0 2.5-1.12 2.5-2.5v-12c0-.03-.01-.06-.03-.07zM16.5 5l1.53 1.53L18 7H7.5C6.67 7 6 6.33 6 5.5S6.67 4 7.5 4h9V5zm.5 13.5c0 .83-.67 1.5-1.5 1.5h-9c-.83 0-1.5-.67-1.5-1.5v-10H17v10z',
  'person-outline': 'M12 4C9.79 4 8 5.79 8 8s1.79 4 4 4 4-1.79 4-4-1.79-4-4-4zm0 1.9c1.16 0 2.1.94 2.1 2.1 0 1.16-.94 2.1-2.1 2.1-1.16 0-2.1-.94-2.1-2.1 0-1.16.94-2.1 2.1-2.1zM12 13c-2.67 0-8 1.34-8 4v3h16v-3c0-2.66-5.33-4-8-4zm0 1.9c2.97 0 6.1 1.46 6.1 2.1v1.1H5.9V17c0-.64 3.13-2.1 6.1-2.1z',
  'information-circle-outline': 'M11 17h2v-6h-2v6zm1-15C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zM11 9h2V7h-2v2z',
  
  // MaterialCommunityIcons
  'robot-happy-outline': 'M22 14h-1c0-3.87-3.13-7-7-7h-1V5.73c.45-.27.75-.76.75-1.32C13.75 3.6 13.14 3 12.38 3h-.76C10.86 3 10.25 3.6 10.25 4.41c0 .56.3 1.05.75 1.32V7h-1c-3.87 0-7 3.13-7 7H2v3h1v4h18v-4h1v-3zM12 15c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3zm-7 1h14v1H5v-1z',
};

interface IoniconsMockProps {
  name: string;
  size?: number;
  color?: string;
  style?: React.CSSProperties;
}

// A simple component that renders SVG icons
export const IoniconsMock: React.FC<{
  name: string;
  size?: number;
  color?: string;
  style?: React.CSSProperties;
}> = ({ 
  name, 
  size = 24, 
  color = 'currentColor',
  style = {}
}) => {
  // If we don't have this icon, render a placeholder
  if (!iconPaths[name]) {
    console.warn(`Icon "${name}" not found in mock Ionicons`);
    return (
      <div 
        style={{ 
          width: size, 
          height: size, 
          backgroundColor: 'lightgray',
          borderRadius: '50%',
          display: 'inline-block',
          ...style
        }} 
      />
    );
  }

  return (
    <svg 
      width={size} 
      height={size} 
      viewBox="0 0 24 24" 
      fill={color}
      stroke="none"
      style={style}
    >
      <path d={iconPaths[name]} />
    </svg>
  );
};

export default IoniconsMock;