import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { perforatedSquarePath, Stamp } from './Stamp';

function numbers(path: string): number[] {
  return (path.match(/-?\d+(?:\.\d+)?/g) ?? []).map(Number);
}

describe('perforatedSquarePath', () => {
  it('closes the outline and starts at the top-left corner', () => {
    const d = perforatedSquarePath(7);
    expect(d.startsWith('M0 0')).toBe(true);
    expect(d.endsWith('Z')).toBe(true);
  });

  it('bites once per tooth on each of the four edges', () => {
    for (const teeth of [5, 7, 9]) {
      expect((perforatedSquarePath(teeth).match(/A/g) ?? []).length).toBe(teeth * 4);
    }
  });

  it('stays inside the 100 x 100 box', () => {
    for (const teeth of [5, 7, 9]) {
      for (const value of numbers(perforatedSquarePath(teeth))) {
        expect(value).toBeGreaterThanOrEqual(0);
        expect(value).toBeLessThanOrEqual(100);
      }
    }
  });

  it('scales its bites with the spacing, so a stamp looks the same at every tooth count', () => {
    const radius = (teeth: number) =>
      numbers((perforatedSquarePath(teeth).match(/A[\d. ]+/) ?? [''])[0])[0];
    expect(radius(5)).toBeGreaterThan(radius(9) as number);
  });
});

describe('Stamp', () => {
  it('draws people as perforated stamps and services as postmarks', () => {
    const person = render(<Stamp name="Maya Okafor" ink={6} />);
    expect(person.container.querySelector('[data-stamp="perforated"]')).not.toBeNull();
    const service = render(<Stamp name="DHL Parcel" ink={2} kind="service" />);
    expect(service.container.querySelector('[data-stamp="postmark"]')).not.toBeNull();
  });

  it('shows initials, in the right colour', () => {
    const { container } = render(<Stamp name="Maya Okafor" ink={6} />);
    expect(container.querySelector('text')?.textContent).toBe('MO');
    expect(container.querySelector('svg')?.getAttribute('style')).toContain('var(--stamp-6)');
  });

  it('is hidden from assistive technology because the name is printed beside it', () => {
    const { container } = render(<Stamp name="Maya Okafor" ink={6} />);
    expect(container.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('adds postmark waves only on large stamps', () => {
    const small = render(<Stamp name="Maya Okafor" ink={6} size={36} />);
    expect(small.container.querySelectorAll('path').length).toBe(1);
    const large = render(<Stamp name="Maya Okafor" ink={6} size={72} />);
    expect(large.container.querySelectorAll('path').length).toBeGreaterThan(1);
  });

  it('uses the text colour for you', () => {
    const { container } = render(<Stamp name="You" ink={0} kind="self" />);
    expect(container.querySelector('svg')?.getAttribute('style')).toContain('var(--text)');
  });
});
