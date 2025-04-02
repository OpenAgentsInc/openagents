// Type declarations for @expo/vector-icons to fix TypeScript errors
declare module '@expo/vector-icons' {
  import * as React from 'react';

  interface IconProps {
    name: string;
    size?: number;
    color?: string;
    style?: any;
  }

  // Fix for Ionicons
  export class Ionicons extends React.Component<IconProps> {}
  
  // Fix for MaterialCommunityIcons
  export class MaterialCommunityIcons extends React.Component<IconProps> {}
  
  // Fix for MaterialIcons with glyphMap
  export class MaterialIcons extends React.Component<IconProps> {
    static glyphMap: Record<string, string>;
  }
  
  // Include all other potential icon families that might be used
  export class AntDesign extends React.Component<IconProps> {}
  export class Entypo extends React.Component<IconProps> {}
  export class EvilIcons extends React.Component<IconProps> {}
  export class Feather extends React.Component<IconProps> {}
  export class FontAwesome extends React.Component<IconProps> {}
  export class FontAwesome5 extends React.Component<IconProps> {}
  export class Fontisto extends React.Component<IconProps> {}
  export class Foundation extends React.Component<IconProps> {}
  export class Octicons extends React.Component<IconProps> {}
  export class SimpleLineIcons extends React.Component<IconProps> {}
  export class Zocial extends React.Component<IconProps> {}
}

// Fix for createIconSet import
declare module '@expo/vector-icons/build/createIconSet' {
  import * as React from 'react';
  
  export interface Icon {
    glyphMap: Record<string, string>;
  }
}