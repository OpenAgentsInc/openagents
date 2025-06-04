// Types for Pane dropdown menu system

export interface PaneDropdownItemAction {
  label: string;
  action: (event?: React.MouseEvent) => void;
  disabled?: boolean;
  icon?: React.ReactNode;
}

export interface PaneDropdownItemSeparator {
  type: 'separator';
}

export interface PaneDropdownItemLabel {
  type: 'label';
  label: string;
}

export interface PaneDropdownItemGroup {
  type: 'group';
  label?: string;
  items: PaneDropdownItem[];
}

export interface PaneDropdownItemSub {
  type: 'submenu';
  label: string;
  icon?: React.ReactNode;
  items: PaneDropdownItem[];
}

export type PaneDropdownItem = 
  | PaneDropdownItemAction
  | PaneDropdownItemSeparator
  | PaneDropdownItemLabel
  | PaneDropdownItemGroup
  | PaneDropdownItemSub;

export interface PaneHeaderMenu {
  id: string;
  triggerLabel: string;
  items: PaneDropdownItem[];
}