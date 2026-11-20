import { describe, expect, it } from 'vitest';
import type { Recurrence } from './bindings';
import { describeRecurrence, minutesToTime, ordinal, timeToMinutes } from './routines';

const rec = (over: Partial<Recurrence>): Recurrence => ({
  kind: 'daily',
  minutes: 540,
  days: [],
  day_of_month: 1,
  ...over,
});

describe('describeRecurrence', () => {
  it('says when in plain words', () => {
    expect(describeRecurrence(rec({}))).toBe('Every day at 09:00');
    expect(describeRecurrence(rec({ kind: 'weekdays', minutes: 8 * 60 + 5 }))).toBe(
      'Every weekday at 08:05',
    );
    expect(describeRecurrence(rec({ kind: 'weekly', days: [1] }))).toBe('Every Monday at 09:00');
    expect(describeRecurrence(rec({ kind: 'weekly', days: [4, 1] }))).toBe(
      'Every Monday and Thursday at 09:00',
    );
    expect(describeRecurrence(rec({ kind: 'weekly', days: [5, 1, 3, 3] }))).toBe(
      'Every Monday, Wednesday and Friday at 09:00',
    );
    expect(describeRecurrence(rec({ kind: 'weekly', days: [] }))).toBe('Every Monday at 09:00');
    expect(describeRecurrence(rec({ kind: 'monthly', day_of_month: 1 }))).toBe(
      'On the 1st of every month at 09:00',
    );
    expect(describeRecurrence(rec({ kind: 'monthly', day_of_month: 31 }))).toBe(
      'On the last day of every month at 09:00',
    );
    expect(describeRecurrence(rec({ kind: 'monthly', day_of_month: 30 }))).toContain(
      '30th of every month, or the last day',
    );
    expect(describeRecurrence(rec({ kind: 'manual' }))).toBe('Only when you ask');
  });
});

describe('times and ordinals', () => {
  it('converts between minutes and a clock time', () => {
    expect(minutesToTime(0)).toBe('00:00');
    expect(minutesToTime(9 * 60 + 5)).toBe('09:05');
    expect(minutesToTime(23 * 60 + 59)).toBe('23:59');
    expect(timeToMinutes('07:30')).toBe(450);
    expect(timeToMinutes('')).toBe(0);
  });

  it('writes ordinals, including the teens', () => {
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 30].map(ordinal)).toEqual([
      '1st',
      '2nd',
      '3rd',
      '4th',
      '11th',
      '12th',
      '13th',
      '21st',
      '22nd',
      '23rd',
      '30th',
    ]);
  });
});
