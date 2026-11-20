export type PageAnim = 'fold' | 'slide' | 'rise' | 'zoom' | 'fade' | 'none';
export type DialogAnim = 'pop' | 'rise' | 'fade' | 'none';
export type ToastAnim = 'rise' | 'slide' | 'fade' | 'none';
export type StampAnim = 'full' | 'fade' | 'none';

export interface Animations {
  page: PageAnim;
  dialog: DialogAnim;
  toast: ToastAnim;
  stamp: StampAnim;
  strand: boolean;
  flight: boolean;
}

export const DEFAULT_ANIMATIONS: Animations = {
  page: 'fold',
  dialog: 'pop',
  toast: 'rise',
  stamp: 'full',
  strand: true,
  flight: true,
};

export const PAGE_OPTIONS: { value: PageAnim; label: string; hint: string }[] = [
  { value: 'fold', label: 'Fold', hint: 'The new page swings open like a letter' },
  { value: 'slide', label: 'Slide', hint: 'The new page slides in from the side' },
  { value: 'rise', label: 'Rise', hint: 'The new page lifts into place' },
  { value: 'zoom', label: 'Zoom', hint: 'The new page grows from slightly smaller' },
  { value: 'fade', label: 'Fade', hint: 'A quick crossfade' },
  { value: 'none', label: 'None', hint: 'Switch at once' },
];

export const DIALOG_OPTIONS: { value: DialogAnim; label: string; hint: string }[] = [
  { value: 'pop', label: 'Pop', hint: 'Grows in with a small bounce' },
  { value: 'rise', label: 'Rise', hint: 'Slides up from below' },
  { value: 'fade', label: 'Fade', hint: 'Fades in' },
  { value: 'none', label: 'None', hint: 'Appears at once' },
];

export const TOAST_OPTIONS: { value: ToastAnim; label: string; hint: string }[] = [
  { value: 'rise', label: 'Rise', hint: 'Lifts in from the bottom' },
  { value: 'slide', label: 'Slide', hint: 'Slides in from the side' },
  { value: 'fade', label: 'Fade', hint: 'Fades in' },
  { value: 'none', label: 'None', hint: 'Appears at once' },
];

export const STAMP_OPTIONS: { value: StampAnim; label: string; hint: string }[] = [
  { value: 'full', label: 'Stamp', hint: 'A postmark lands with a ring' },
  { value: 'fade', label: 'Fade', hint: 'A postmark fades in' },
  { value: 'none', label: 'None', hint: 'No postmark' },
];
