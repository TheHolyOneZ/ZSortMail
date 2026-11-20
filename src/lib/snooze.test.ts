import { describe, expect, it } from 'vitest';
import { snoozeChoices } from './snooze';

const WED_MORNING = new Date(2026, 10, 18, 10, 7).getTime();

describe('snoozeChoices', () => {
  it('offers later today, tomorrow morning, the weekend and next week', () => {
    const choices = snoozeChoices(WED_MORNING);
    expect(choices.map((c) => c.label)).toEqual([
      'Later today',
      'Tomorrow morning',
      'This weekend',
      'Next week',
    ]);
    const later = new Date(choices[0]?.at ?? 0);
    expect([later.getHours(), later.getMinutes()]).toEqual([13, 15]);
    const tomorrow = new Date(choices[1]?.at ?? 0);
    expect([tomorrow.getDate(), tomorrow.getHours()]).toEqual([19, 8]);
    const weekend = new Date(choices[2]?.at ?? 0);
    expect([weekend.getDay(), weekend.getHours()]).toEqual([6, 9]);
    const monday = new Date(choices[3]?.at ?? 0);
    expect([monday.getDay(), monday.getHours()]).toEqual([1, 8]);
  });

  it('leaves out later today when the evening has gone, and never offers the past', () => {
    const lateNight = new Date(2026, 10, 18, 22, 30).getTime();
    const choices = snoozeChoices(lateNight);
    expect(choices.map((c) => c.label)).not.toContain('Later today');
    expect(choices.every((c) => c.at > lateNight)).toBe(true);
  });

  it('does not offer a weekend or next week that is the same morning as tomorrow', () => {
    const thursday = new Date(2026, 10, 19, 15, 0).getTime();
    expect(snoozeChoices(thursday).map((c) => c.label)).toContain('This weekend');
    const friday = new Date(2026, 10, 20, 15, 0).getTime();
    expect(snoozeChoices(friday).map((c) => c.label)).not.toContain('This weekend');
    const saturday = new Date(2026, 10, 21, 12, 0).getTime();
    expect(snoozeChoices(saturday).map((c) => c.label)).not.toContain('This weekend');
    const sunday = new Date(2026, 10, 22, 12, 0).getTime();
    expect(snoozeChoices(sunday).map((c) => c.label)).not.toContain('Next week');
  });
});
