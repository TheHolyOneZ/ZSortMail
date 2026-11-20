export interface Rgb {
  r: number;
  g: number;
  b: number;
}

const clamp = (n: number, lo = 0, hi = 255) => Math.min(hi, Math.max(lo, n));

export function parseColor(input: string): Rgb | null {
  const text = input.trim().toLowerCase();
  if (!text) return null;
  const hex = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/.exec(text);
  if (hex?.[1]) {
    const h = hex[1].length === 3 ? [...hex[1]].map((c) => c + c).join('') : hex[1];
    return {
      r: Number.parseInt(h.slice(0, 2), 16),
      g: Number.parseInt(h.slice(2, 4), 16),
      b: Number.parseInt(h.slice(4, 6), 16),
    };
  }
  const nums = (body: string) => body.split(/[\s,/]+/).filter(Boolean);
  const rgb = /^rgba?\(([^)]*)\)$/.exec(text);
  const bare = /^\d{1,3}[\s,]+\d{1,3}[\s,]+\d{1,3}$/.test(text) ? text : null;
  const parts = rgb?.[1] ? nums(rgb[1]) : bare ? nums(bare) : null;
  if (parts && parts.length >= 3) {
    const v = parts
      .slice(0, 3)
      .map((p) => (p.endsWith('%') ? (Number.parseFloat(p) / 100) * 255 : Number.parseFloat(p)));
    if (v.every((n) => Number.isFinite(n) && n >= 0 && n <= 255))
      return {
        r: Math.round(v[0] as number),
        g: Math.round(v[1] as number),
        b: Math.round(v[2] as number),
      };
    return null;
  }
  const hsl = /^hsla?\(([^)]*)\)$/.exec(text);
  if (hsl?.[1]) {
    const [h, s, l] = nums(hsl[1]);
    const hue = Number.parseFloat(h ?? '');
    const sat = Number.parseFloat(s ?? '') / 100;
    const lig = Number.parseFloat(l ?? '') / 100;
    if (![hue, sat, lig].every(Number.isFinite) || sat < 0 || sat > 1 || lig < 0 || lig > 1)
      return null;
    const c = (1 - Math.abs(2 * lig - 1)) * sat;
    const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
    const m = lig - c / 2;
    const sector = Math.floor((((hue % 360) + 360) % 360) / 60);
    const [r = 0, g = 0, b = 0] = ([
      [c, x, 0],
      [x, c, 0],
      [0, c, x],
      [0, x, c],
      [x, 0, c],
      [c, 0, x],
    ][sector] ?? [0, 0, 0]) as number[];
    return {
      r: Math.round((r + m) * 255),
      g: Math.round((g + m) * 255),
      b: Math.round((b + m) * 255),
    };
  }
  return null;
}

export function toHex({ r, g, b }: Rgb): string {
  return `#${[r, g, b].map((n) => clamp(Math.round(n)).toString(16).padStart(2, '0')).join('')}`;
}

export function toRgbText({ r, g, b }: Rgb): string {
  return `rgb(${r}, ${g}, ${b})`;
}

export function luminance({ r, g, b }: Rgb): number {
  const lin = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

export function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

export function mix(a: Rgb, b: Rgb, amount: number): Rgb {
  const k = clamp(amount, 0, 1);
  return { r: a.r + (b.r - a.r) * k, g: a.g + (b.g - a.g) * k, b: a.b + (b.b - a.b) * k };
}

export function isDark(c: Rgb): boolean {
  return luminance(c) < 0.18;
}
