import { describe, expect, it } from 'vitest';
import {
  addProfile,
  adopt,
  commit,
  copyFrom,
  NO_PROFILES,
  type ProfilesState,
  profileKey,
  setSync,
  snapshotOf,
  switchTo,
} from './profiles';

const defaults = { theme: 'system', density: 'comfortable', tidyLetters: true };
const live = (over: Record<string, unknown> = {}) => ({ ...defaults, panes: 1, ...over });

function two(): ProfilesState {
  const first = adopt(NO_PROFILES, ['A@x.test'], live({ theme: 'ink' })).profiles;
  return addProfile(first, live({ theme: 'ink' }), defaults, 'b@x.test', {
    copy: false,
    sync: false,
  }).profiles;
}

describe('profiles', () => {
  it('keeps only what a profile owns', () => {
    expect(snapshotOf(live())).toEqual(defaults);
    expect(profileKey('  A@X.test ')).toBe('a@x.test');
  });

  it('adopts the first mailbox silently with the settings you have', () => {
    const { profiles, pending } = adopt(
      NO_PROFILES,
      ['a@x.test', 'b@x.test'],
      live({ theme: 'ink' }),
    );
    expect(pending).toEqual([]);
    expect(profiles.active).toBe('a@x.test');
    expect((profiles.saved['b@x.test'] as { theme: string }).theme).toBe('ink');
  });

  it('asks about a mailbox that appears later, and forgets removed ones', () => {
    const first = adopt(NO_PROFILES, ['a@x.test'], live()).profiles;
    expect(adopt(first, ['a@x.test', 'c@x.test'], live()).pending).toEqual(['c@x.test']);
    const gone = adopt(two(), ['b@x.test'], live());
    expect(Object.keys(gone.profiles.saved)).toEqual(['b@x.test']);
    expect(gone.profiles.active).toBe('b@x.test');
  });

  it('a new profile can start from your settings or from the defaults', () => {
    const base = adopt(NO_PROFILES, ['a@x.test'], live({ theme: 'ink' })).profiles;
    const copied = addProfile(base, live({ theme: 'ink' }), defaults, 'b@x.test', {
      copy: true,
      sync: false,
    });
    expect((copied.apply as { theme: string }).theme).toBe('ink');
    const fresh = addProfile(base, live({ theme: 'ink' }), defaults, 'b@x.test', {
      copy: false,
      sync: false,
    });
    expect((fresh.apply as { theme: string }).theme).toBe('system');
    expect(fresh.profiles.active).toBe('b@x.test');
  });

  it('switching saves where you were and loads the other profile', () => {
    const ps = two();
    const r = switchTo(ps, live({ theme: 'paper-blue' }), 'a@x.test');
    expect((r.profiles.saved['b@x.test'] as { theme: string }).theme).toBe('paper-blue');
    expect((r.apply as { theme: string }).theme).toBe('ink');
    expect(switchTo(r.profiles, live(), 'nobody@x.test').apply).toBeNull();
  });

  it('with sync on, every profile shares one set and switching loads nothing', () => {
    const ps = setSync(two(), live({ theme: 'sand' }), true);
    for (const snap of Object.values(ps.saved))
      expect((snap as { theme: string }).theme).toBe('sand');
    const r = switchTo(ps, live({ theme: 'sand' }), 'a@x.test');
    expect(r.apply).toBeNull();
    expect(r.profiles.active).toBe('a@x.test');
    expect(
      (commit(r.profiles, live({ theme: 'moss' })).saved['b@x.test'] as { theme: string }).theme,
    ).toBe('moss');
    const off = setSync(r.profiles, live({ theme: 'moss' }), false);
    expect(off.sync).toBe(false);
  });

  it('copies another profile onto the active one', () => {
    const ps = two();
    const r = copyFrom(ps, live({ theme: 'x' }), 'a@x.test');
    expect((r.apply as { theme: string }).theme).toBe('ink');
    expect((r.profiles.saved['b@x.test'] as { theme: string }).theme).toBe('ink');
    expect(copyFrom(ps, live(), 'b@x.test').apply).toBeNull();
  });
});
