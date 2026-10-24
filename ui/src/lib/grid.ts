// Columns of a VirtualGrid.

export interface Column {
  id: string;
  label: string;
  /** CSS track size: `2fr`, `8rem`… */
  width: string;
  align?: 'start' | 'end';
  sortable?: boolean;
}
