import { useEffect } from 'react';
import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import { type Animations, DEFAULT_ANIMATIONS } from '@/lib/animations';
import type { PeopleFilter } from '@/lib/bindings';
import { applyCustom, CUSTOM_PREFIX, type CustomTheme } from '@/lib/customTheme';
import { direction, type LocalePreference, resolveLocale } from '@/lib/i18n';
import { DEFAULT_KEYS, type Keys } from '@/lib/keymap';
import { isTheme, type ThemePreference } from '@/lib/themes';
import type { MotionPreference } from '@/motion/governor';
import {
  addProfile,
  adopt,
  copyFrom,
  NO_PROFILES,
  type ProfilesState,
  type Snapshot,
  setSync,
  snapshotOf,
  switchTo,
} from './profiles';
import { cleanSidebar, DEFAULT_SIDEBAR, type SidebarLayout } from './sidebar';

export type { ThemePreference };

export const DEFAULT_PANES = { people: 280, conversations: 320, rail: 212 } as const;
export const PANE_LIMITS = {
  people: { min: 220, max: 440 },
  conversations: { min: 260, max: 520 },
  rail: { min: 168, max: 320 },
} as const;
export type Density = 'comfortable' | 'compact';
export type PeopleView = 'list' | 'wall';
export type StartPage = 'people' | 'mail' | 'today';
export type RowActions = 'hover' | 'always';

export const START_ROUTES: Record<StartPage, '/people' | '/river' | '/today'> = {
  people: '/people',
  mail: '/river',
  today: '/today',
};

export function isStartPage(value: unknown): value is StartPage {
  return value === 'people' || value === 'mail' || value === 'today';
}

export function startRoute(): '/people' | '/river' | '/today' {
  const page = useUi.getState().startPage;
  return START_ROUTES[isStartPage(page) ? page : 'people'];
}

interface UiState {
  theme: ThemePreference;
  density: Density;
  motion: MotionPreference;
  peopleView: PeopleView;
  customThemes: CustomTheme[];
  saveCustomTheme: (theme: CustomTheme) => void;
  deleteCustomTheme: (id: string) => void;
  draftTheme: Pick<CustomTheme, 'base' | 'colors'> | null;
  setDraftTheme: (theme: Pick<CustomTheme, 'base' | 'colors'> | null) => void;
  language: LocalePreference;
  setLanguage: (language: LocalePreference) => void;
  keys: Keys;
  setKeys: (change: Partial<Keys>) => void;
  anim: Animations;
  setAnim: (change: Partial<Animations>) => void;
  paneWidths: { people: number; conversations: number; rail: number };
  setPaneWidth: (pane: 'people' | 'conversations' | 'rail', width: number) => void;
  resetPaneWidth: (pane: 'people' | 'conversations' | 'rail') => void;
  railOpen: boolean;
  tidyLetters: boolean;
  categoriesFirst: boolean;
  startPage: StartPage;
  rowActions: RowActions;
  sidebar: SidebarLayout;
  profiles: ProfilesState;
  adoptAccounts: (emails: string[]) => string[];
  switchProfile: (email: string) => void;
  addProfile: (email: string, options: { copy: boolean; sync: boolean }) => void;
  setProfileSync: (on: boolean) => void;
  copyProfile: (email: string) => void;
  peopleFilter: PeopleFilter;
  peopleCategory: string | null;
  peopleQuery: string;
  setTheme: (theme: ThemePreference) => void;
  setDensity: (density: Density) => void;
  setMotion: (motion: MotionPreference) => void;
  setPeopleView: (view: PeopleView) => void;
  setRailOpen: (open: boolean) => void;
  setTidyLetters: (tidy: boolean) => void;
  setCategoriesFirst: (on: boolean) => void;
  setStartPage: (page: StartPage) => void;
  setRowActions: (mode: RowActions) => void;
  setSidebar: (layout: SidebarLayout) => void;
  setPeopleFilter: (filter: PeopleFilter) => void;
  setPeopleCategory: (category: string | null) => void;
  setPeopleQuery: (query: string) => void;
}

function defaultSettings(): Snapshot {
  return snapshotOf({
    theme: 'system',
    customThemes: [],
    density: 'comfortable',
    motion: 'auto',
    peopleView: 'list',
    anim: DEFAULT_ANIMATIONS,
    keys: { ...DEFAULT_KEYS, overrides: {} },
    language: 'en',
    tidyLetters: true,
    categoriesFirst: true,
    startPage: 'people',
    rowActions: 'hover',
    sidebar: DEFAULT_SIDEBAR,
  });
}

export const useUi = create<UiState>()(
  persist(
    (set, get) => ({
      theme: 'system',
      density: 'comfortable',
      motion: 'auto',
      peopleView: 'list',
      railOpen: true,
      customThemes: [],
      saveCustomTheme: (theme) =>
        set((st) => ({
          customThemes: st.customThemes.some((t) => t.id === theme.id)
            ? st.customThemes.map((t) => (t.id === theme.id ? theme : t))
            : [...st.customThemes, theme],
        })),
      deleteCustomTheme: (id) =>
        set((st) => ({
          customThemes: st.customThemes.filter((t) => t.id !== id),
          theme: st.theme === `${CUSTOM_PREFIX}${id}` ? 'system' : st.theme,
        })),
      draftTheme: null,
      setDraftTheme: (draftTheme) => set({ draftTheme }),
      language: 'en',
      setLanguage: (language) => set({ language }),
      keys: { ...DEFAULT_KEYS, overrides: {} },
      setKeys: (change) => set((st) => ({ keys: { ...st.keys, ...change } })),
      anim: { ...DEFAULT_ANIMATIONS },
      setAnim: (change) => set((st) => ({ anim: { ...st.anim, ...change } })),
      paneWidths: { ...DEFAULT_PANES },
      setPaneWidth: (pane, width) =>
        set((st) => ({ paneWidths: { ...st.paneWidths, [pane]: Math.round(width) } })),
      resetPaneWidth: (pane) =>
        set((st) => ({ paneWidths: { ...st.paneWidths, [pane]: DEFAULT_PANES[pane] } })),
      tidyLetters: true,
      categoriesFirst: true,
      startPage: 'people',
      rowActions: 'hover',
      sidebar: DEFAULT_SIDEBAR,
      profiles: NO_PROFILES,
      adoptAccounts: (emails) => {
        const { profiles, pending } = adopt(get().profiles, emails, get() as never);
        set({ profiles });
        return pending;
      },
      switchProfile: (email) => {
        const r = switchTo(get().profiles, get() as never, email);
        set({ profiles: r.profiles, ...(r.apply ?? {}) } as Partial<UiState>);
      },
      addProfile: (email, options) => {
        const r = addProfile(get().profiles, get() as never, defaultSettings(), email, options);
        set({ profiles: r.profiles, ...(r.apply ?? {}) } as Partial<UiState>);
      },
      setProfileSync: (on) => set({ profiles: setSync(get().profiles, get() as never, on) }),
      copyProfile: (email) => {
        const r = copyFrom(get().profiles, get() as never, email);
        set({ profiles: r.profiles, ...(r.apply ?? {}) } as Partial<UiState>);
      },
      peopleFilter: 'all',
      peopleCategory: null,
      peopleQuery: '',
      setTheme: (theme) => set({ theme }),
      setDensity: (density) => set({ density }),
      setMotion: (motion) => set({ motion }),
      setPeopleView: (peopleView) => set({ peopleView }),
      setRailOpen: (railOpen) => set({ railOpen }),
      setTidyLetters: (tidyLetters) => set({ tidyLetters }),
      setCategoriesFirst: (categoriesFirst) => set({ categoriesFirst }),
      setStartPage: (startPage) => set({ startPage }),
      setRowActions: (rowActions) => set({ rowActions }),
      setSidebar: (sidebar) => set({ sidebar }),
      setPeopleFilter: (peopleFilter) => set({ peopleFilter }),
      setPeopleCategory: (peopleCategory) => set({ peopleCategory }),
      setPeopleQuery: (peopleQuery) => set({ peopleQuery }),
    }),
    {
      name: 'zsm-ui',
      storage: createJSONStorage(() => localStorage),
      merge: (saved, current) => {
        const merged = { ...current, ...(saved as Partial<UiState>) };
        merged.paneWidths = { ...DEFAULT_PANES, ...(merged.paneWidths ?? {}) };
        merged.anim = { ...DEFAULT_ANIMATIONS, ...(merged.anim ?? {}) };
        merged.keys = {
          ...DEFAULT_KEYS,
          ...(merged.keys ?? {}),
          overrides: merged.keys?.overrides ?? {},
        };
        merged.sidebar = cleanSidebar(merged.sidebar);
        merged.profiles = { ...NO_PROFILES, ...(merged.profiles ?? {}) };
        merged.customThemes = Array.isArray(merged.customThemes) ? merged.customThemes : [];
        if (!isStartPage(merged.startPage)) merged.startPage = 'people';
        if (merged.rowActions !== 'always') merged.rowActions = 'hover';
        if (!isTheme(merged.theme)) merged.theme = 'system';
        if (
          merged.theme.startsWith(CUSTOM_PREFIX) &&
          !merged.customThemes.some((t) => `${CUSTOM_PREFIX}${t.id}` === merged.theme)
        ) {
          merged.theme = 'system';
        }
        return merged;
      },
      partialize: (s) => ({
        theme: s.theme,
        density: s.density,
        motion: s.motion,
        peopleView: s.peopleView,
        railOpen: s.railOpen,
        paneWidths: s.paneWidths,
        anim: s.anim,
        keys: s.keys,
        language: s.language,
        customThemes: s.customThemes,
        tidyLetters: s.tidyLetters,
        categoriesFirst: s.categoriesFirst,
        startPage: s.startPage,
        rowActions: s.rowActions,
        sidebar: s.sidebar,
        profiles: s.profiles,
      }),
    },
  ),
);

export const rowHeights = {
  comfortable: { people: 60, convo: 72 },
  compact: { people: 48, convo: 60 },
} as const;

export function useApplyUiPreferences(): void {
  const theme = useUi((s) => s.theme);
  const density = useUi((s) => s.density);
  const anim = useUi((s) => s.anim);
  const language = useUi((s) => s.language);
  useEffect(() => {
    const locale = resolveLocale(language, navigator.languages);
    document.documentElement.lang = locale;
    document.documentElement.dir = direction(locale);
  }, [language]);
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.dialogAnim = anim.dialog;
    root.dataset.toastAnim = anim.toast;
  }, [anim.dialog, anim.toast]);
  const customThemes = useUi((s) => s.customThemes);
  const draft = useUi((s) => s.draftTheme);
  useEffect(() => {
    const root = document.documentElement;
    const own = theme.startsWith(CUSTOM_PREFIX)
      ? customThemes.find((t) => `${CUSTOM_PREFIX}${t.id}` === theme)
      : undefined;
    const custom = draft ?? own ?? null;
    if (custom) {
      root.dataset.theme = custom.base;
      applyCustom(root, custom);
    } else {
      applyCustom(root, null);
      if (theme === 'system') delete root.dataset.theme;
      else root.dataset.theme = theme;
    }
    root.dataset.density = density;
  }, [theme, density, customThemes, draft]);
}
