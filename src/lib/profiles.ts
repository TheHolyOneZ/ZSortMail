export const PROFILE_KEYS = [
  'theme',
  'customThemes',
  'density',
  'motion',
  'peopleView',
  'anim',
  'keys',
  'language',
  'tidyLetters',
  'categoriesFirst',
  'sidebar',
] as const;

export type Snapshot = Record<string, unknown>;

export interface ProfilesState {
  active: string | null;
  sync: boolean;
  saved: Record<string, Snapshot>;
}

export const NO_PROFILES: ProfilesState = { active: null, sync: false, saved: {} };

export const profileKey = (email: string): string => email.trim().toLowerCase();

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;

export function snapshotOf(live: Record<string, unknown>): Snapshot {
  const out: Snapshot = {};
  for (const key of PROFILE_KEYS) if (key in live) out[key] = clone(live[key]);
  return out;
}

export function commit(ps: ProfilesState, live: Record<string, unknown>): ProfilesState {
  if (ps.active === null) return ps;
  const snap = snapshotOf(live);
  if (ps.sync) {
    const saved = Object.fromEntries(Object.keys(ps.saved).map((k) => [k, clone(snap)]));
    return { ...ps, saved };
  }
  return { ...ps, saved: { ...ps.saved, [ps.active]: snap } };
}

export function adopt(
  ps: ProfilesState,
  emails: string[],
  live: Record<string, unknown>,
): { profiles: ProfilesState; pending: string[] } {
  const keys = [...new Set(emails.map(profileKey))];
  const kept = Object.fromEntries(Object.entries(ps.saved).filter(([k]) => keys.includes(k)));
  if (Object.keys(ps.saved).length === 0) {
    const saved = Object.fromEntries(keys.map((k) => [k, snapshotOf(live)]));
    return { profiles: { ...ps, active: keys[0] ?? null, saved }, pending: [] };
  }
  const active = ps.active && ps.active in kept ? ps.active : (Object.keys(kept)[0] ?? null);
  return {
    profiles: { ...ps, active, saved: kept },
    pending: keys.filter((k) => !(k in kept)),
  };
}

export function switchTo(
  ps: ProfilesState,
  live: Record<string, unknown>,
  email: string,
): { profiles: ProfilesState; apply: Snapshot | null } {
  const key = profileKey(email);
  if (!(key in ps.saved) || key === ps.active) return { profiles: ps, apply: null };
  if (ps.sync) return { profiles: { ...ps, active: key }, apply: null };
  const saved = commit(ps, live).saved;
  return { profiles: { ...ps, active: key, saved }, apply: clone(saved[key] as Snapshot) };
}

export function addProfile(
  ps: ProfilesState,
  live: Record<string, unknown>,
  defaults: Snapshot,
  email: string,
  options: { copy: boolean; sync: boolean },
): { profiles: ProfilesState; apply: Snapshot | null } {
  const key = profileKey(email);
  const current = commit(ps, live);
  const fresh = options.copy || options.sync ? snapshotOf(live) : clone(defaults);
  const saved = { ...current.saved, [key]: fresh };
  const profiles: ProfilesState = { active: key, sync: options.sync || ps.sync, saved };
  if (profiles.sync) {
    const shared = snapshotOf(live);
    return {
      profiles: {
        ...profiles,
        saved: Object.fromEntries(Object.keys(saved).map((k) => [k, clone(shared)])),
      },
      apply: null,
    };
  }
  return { profiles, apply: clone(fresh) };
}

export function setSync(
  ps: ProfilesState,
  live: Record<string, unknown>,
  sync: boolean,
): ProfilesState {
  return commit({ ...ps, sync }, live);
}

export function copyFrom(
  ps: ProfilesState,
  live: Record<string, unknown>,
  email: string,
): { profiles: ProfilesState; apply: Snapshot | null } {
  const key = profileKey(email);
  const source = ps.saved[key];
  if (!source || key === ps.active) return { profiles: ps, apply: null };
  const snap = clone(source);
  return { profiles: commit(ps, { ...live, ...snap }), apply: snap };
}
