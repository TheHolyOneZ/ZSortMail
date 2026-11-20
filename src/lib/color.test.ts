import { describe, expect, it } from 'vitest';
import { contrast, isDark, mix, parseColor, toHex, toRgbText } from './color';

describe('parseColor', () => {
  it('reads the ways people write colours', () => {
    const navy = { r: 26, g: 43, b: 60 };
    for (const text of [
      '#1a2b3c',
      '1A2B3C',
      'rgb(26, 43, 60)',
      'rgb(26 43 60)',
      '26 43 60',
      '26,43,60',
      ' #1a2b3c ',
    ]) {
      expect(parseColor(text), text).toEqual(navy);
    }
    expect(parseColor('#fff')).toEqual({ r: 255, g: 255, b: 255 });
    expect(parseColor('#0af')).toEqual({ r: 0, g: 170, b: 255 });
    expect(parseColor('rgb(100%, 0%, 50%)')).toEqual({ r: 255, g: 0, b: 128 });
    expect(parseColor('hsl(0 100% 50%)')).toEqual({ r: 255, g: 0, b: 0 });
    expect(parseColor('hsl(210, 50%, 50%)')).toEqual({ r: 64, g: 128, b: 191 });
  });

  it('refuses what is not a colour', () => {
    for (const text of [
      '',
      'blue',
      '#12',
      '#12345',
      'rgb(300, 0, 0)',
      'rgb(1, 2)',
      'hsl(10 200% 50%)',
      '#gggggg',
    ]) {
      expect(parseColor(text), text).toBeNull();
    }
  });
});

describe('writing and comparing colours', () => {
  it('writes hex and rgb', () => {
    expect(toHex({ r: 26, g: 43, b: 60 })).toBe('#1a2b3c');
    expect(toHex({ r: 300, g: -4, b: 0.4 })).toBe('#ff0000');
    expect(toRgbText({ r: 1, g: 2, b: 3 })).toBe('rgb(1, 2, 3)');
  });

  it('measures contrast the way WCAG does', () => {
    const black = { r: 0, g: 0, b: 0 };
    const white = { r: 255, g: 255, b: 255 };
    expect(contrast(black, white)).toBeCloseTo(21, 0);
    expect(contrast(white, white)).toBeCloseTo(1, 5);
    expect(contrast(white, black)).toBe(contrast(black, white));
    expect(
      contrast(parseColor('#172135') as never, parseColor('#e6eef4') as never),
    ).toBeGreaterThan(10);
  });

  it('mixes and tells light from dark', () => {
    expect(mix({ r: 0, g: 0, b: 0 }, { r: 200, g: 100, b: 50 }, 0.5)).toEqual({
      r: 100,
      g: 50,
      b: 25,
    });
    expect(isDark(parseColor('#0b1421') as never)).toBe(true);
    expect(isDark(parseColor('#e6eef4') as never)).toBe(false);
  });
});
