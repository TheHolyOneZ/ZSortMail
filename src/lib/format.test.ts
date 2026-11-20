import { describe, expect, it } from 'vitest';
import { DAY, HOUR, MINUTE } from './clock';
import {
  elapsedLabel,
  formatListTime,
  formatMessageTime,
  formatSize,
  gapLabel,
  isLongGap,
  plural,
  spanLabel,
  timeGroup,
} from './format';

const NOW = new Date(2026, 10, 18, 10, 0, 0).getTime();
const at = (daysAgo: number, h: number, m: number) =>
  new Date(2026, 10, 18 - daysAgo, h, m, 0).getTime();

describe('formatListTime', () => {
  it('shows the clock time for today', () => {
    expect(formatListTime(at(0, 9, 12), NOW, 'en-GB')).toBe('09:12');
  });

  it('says Yesterday for the previous calendar day, even less than 24 hours ago', () => {
    expect(formatListTime(at(1, 23, 50), NOW, 'en-GB')).toBe('Yesterday');
  });

  it('shows the weekday within the last week', () => {
    expect(formatListTime(at(3, 12, 0), NOW, 'en-GB')).toBe('Sun');
  });

  it('shows day and month in the same year, and adds the year before that', () => {
    expect(formatListTime(at(20, 12, 0), NOW, 'en-GB')).toBe('29 Oct');
    expect(formatListTime(new Date(2025, 2, 4, 12).getTime(), NOW, 'en-GB')).toBe('4 Mar 2025');
  });
});

describe('formatMessageTime', () => {
  it('names the day and the clock', () => {
    expect(formatMessageTime(at(0, 9, 12), NOW, 'en-GB')).toBe('Today 09:12');
    expect(formatMessageTime(at(1, 9, 12), NOW, 'en-GB')).toBe('Yesterday 09:12');
    expect(formatMessageTime(at(3, 9, 12), NOW, 'en-GB')).toBe('Sun 09:12');
    expect(formatMessageTime(at(30, 9, 12), NOW, 'en-GB')).toBe('19 Oct, 09:12');
  });
});

describe('timeGroup', () => {
  it('buckets by calendar day', () => {
    expect(timeGroup(at(0, 1, 0), NOW)).toBe('Today');
    expect(timeGroup(at(1, 23, 0), NOW)).toBe('Yesterday');
    expect(timeGroup(at(4, 8, 0), NOW)).toBe('This week');
    expect(timeGroup(at(12, 8, 0), NOW)).toBe('This month');
    expect(timeGroup(at(90, 8, 0), NOW)).toBe('Earlier');
  });
});

describe('gapLabel', () => {
  it.each([
    [20_000, 'moments later'],
    [40 * MINUTE, '+40 min'],
    [2 * HOUR + 10 * MINUTE, '+2 h'],
    [30 * HOUR, '+30 h'],
    [2 * DAY + 5 * HOUR, '+2 d'],
    [5 * DAY, '5 days later'],
    [16 * DAY, '2 weeks later'],
    [75 * DAY, '3 months later'],
    [900 * DAY, '2 years later'],
  ])('%d ms -> %s', (ms, label) => {
    expect(gapLabel(ms)).toBe(label);
  });

  it('marks gaps of three days or more as long', () => {
    expect(isLongGap(3 * DAY - 1)).toBe(false);
    expect(isLongGap(3 * DAY)).toBe(true);
  });
});

describe('spanLabel', () => {
  it.each([
    [3 * HOUR, 'the same day'],
    [DAY, '1 day'],
    [9 * DAY, '9 days'],
    [23 * DAY, '3 weeks'],
    [75 * DAY, '3 months'],
    [800 * DAY, '2 years'],
  ])('%d ms -> %s', (ms, label) => {
    expect(spanLabel(ms)).toBe(label);
  });
});

describe('plural and formatSize', () => {
  it('pluralizes', () => {
    expect(plural(1, 'message')).toBe('1 message');
    expect(plural(14, 'message')).toBe('14 messages');
    expect(plural(0, 'person', 'people')).toBe('0 people');
  });

  it('uses decimal units like a file manager', () => {
    expect(formatSize(512)).toBe('512 B');
    expect(formatSize(214_000)).toBe('214 KB');
    expect(formatSize(2_400_000)).toBe('2.4 MB');
    expect(formatSize(25_000_000)).toBe('25 MB');
  });
});

describe('elapsedLabel', () => {
  it('shows only as much precision as helps', () => {
    expect(elapsedLabel(10_000)).toBe('moments');
    expect(elapsedLabel(12 * 60_000)).toBe('12 min');
    expect(elapsedLabel(5 * 3_600_000 + 20 * 60_000)).toBe('5 h');
    expect(elapsedLabel(2 * 86_400_000 + 4 * 3_600_000)).toBe('2 d 4 h');
    expect(elapsedLabel(3 * 86_400_000)).toBe('3 d');
    expect(elapsedLabel(21 * 86_400_000)).toBe('3 weeks');
    expect(elapsedLabel(120 * 86_400_000)).toBe('4 months');
  });
});
