import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { THEME_IDS } from '../src/lib/themes';

const NOW = '2026-11-18T10:00:00+01:00';
const url = (hash: string) => `/?now=${encodeURIComponent(NOW)}#${hash}`;

test.use({ viewport: { width: 1360, height: 860 } });

test('every theme meets the contrast rules on the main screens', async ({ browser }) => {
  test.setTimeout(240_000);
  for (const theme of THEME_IDS) {
    const context = await browser.newContext({ viewport: { width: 1360, height: 860 } });
    await context.addInitScript((id) => {
      localStorage.setItem(
        'zsm-ui',
        JSON.stringify({ state: { theme: id, motion: 'off' }, version: 0 }),
      );
    }, theme);
    const page = await context.newPage();
    for (const route of ['/people/1/c/1', '/settings', '/rules']) {
      await page.goto(url(route));
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
      await page.waitForTimeout(500);
      const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa']).analyze();
      const found = results.violations.map(
        (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(' | ')}`,
      );
      expect(found, `${theme} on ${route}`).toEqual([]);
    }
    if (theme === 'nord' || theme === 'sepia')
      await page.screenshot({ path: `test-results/screens/theme-${theme}.png` });
    await context.close();
  }
});

test('a theme is picked from Settings, applies at once and is remembered', async ({ page }) => {
  await page.goto(url('/settings'));
  await page.getByRole('radio', { name: /Forest/ }).check({ force: true });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'forest');
  await page.screenshot({ path: 'test-results/screens/themes-settings.png' });
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'forest');
  await page.getByRole('radio', { name: /System/ }).check({ force: true });
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /.+/);
});

test('animations are chosen in Settings: dialogs and notices follow, and every page style still navigates', async ({
  page,
  browser,
}) => {
  await page.goto(url('/settings'));
  await page.getByRole('combobox', { name: 'Dialogs' }).click();
  await page.getByRole('option', { name: /^Rise/ }).click();
  await expect(page.locator('html')).toHaveAttribute('data-dialog-anim', 'rise');
  await page.getByRole('combobox', { name: 'Notices' }).click();
  await page.getByRole('option', { name: /^None/ }).click();
  await expect(page.locator('html')).toHaveAttribute('data-toast-anim', 'none');
  await page.getByRole('button', { name: 'Try it' }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'This is how a notice appears.' }),
  ).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/animations-settings.png' });
  await page.getByRole('button', { name: 'Back to the defaults' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-dialog-anim', 'pop');

  for (const style of ['fold', 'slide', 'rise', 'zoom', 'fade', 'none']) {
    const context = await browser.newContext({ viewport: { width: 700, height: 800 } });
    await context.addInitScript((s) => {
      localStorage.setItem(
        'zsm-ui',
        JSON.stringify({
          state: { motion: 'full', anim: { page: s }, categoriesFirst: false },
          version: 0,
        }),
      );
    }, style);
    const narrow = await context.newPage();
    await narrow.goto(url('/people'));
    await narrow.getByRole('option', { name: /Maya/ }).click();
    await expect(narrow.getByRole('button', { name: /Q3 launch checklist/ })).toBeVisible();
    await narrow.getByRole('button', { name: /Q3 launch checklist/ }).click();
    await expect(
      narrow.getByRole('heading', { level: 2, name: 'Q3 launch checklist' }),
    ).toBeVisible();
    await narrow.getByRole('button', { name: 'Back to conversations' }).click();
    await expect(narrow.getByRole('button', { name: /Q3 launch checklist/ })).toBeVisible();
    await context.close();
  }
});

test('a key preset is picked in Settings and a single key can be changed, with clashes refused', async ({
  page,
}) => {
  await page.goto(url('/settings'));
  await page.getByRole('combobox', { name: 'Keys' }).click();
  await page.getByRole('option', { name: /^Vim/ }).click();
  const archive = page.getByRole('listitem').filter({ hasText: /^Archive/ });
  await expect(archive.locator('kbd')).toHaveText('x');

  await archive.getByRole('button', { name: /Change the key for Archive/ }).click();
  await page.keyboard.press('r');
  await expect(page.getByText(/already does “Reply”/)).toBeVisible();
  await page.keyboard.press('q');
  await expect(archive.locator('kbd')).toHaveText('q');
  await page.screenshot({ path: 'test-results/screens/keyboard.png', fullPage: true });

  await page.getByRole('link', { name: 'People' }).click();
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();
  await page
    .getByRole('region', { name: 'Conversation', exact: true })
    .click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('x');
  await expect(page.getByRole('status').filter({ hasText: 'Archived.' })).toHaveCount(0);
  await page.keyboard.press('q');
  await expect(page.getByRole('status').filter({ hasText: 'Archived.' })).toBeVisible();
  await page.getByRole('button', { name: 'Undo' }).last().click();

  await page.goto(url('/settings'));
  await page.getByRole('button', { name: /Back to the Vim keys/ }).click();
  await expect(
    page
      .getByRole('listitem')
      .filter({ hasText: /^Archive/ })
      .locator('kbd'),
  ).toHaveText('x');
  await page.getByRole('combobox', { name: 'Keys' }).click();
  await page.getByRole('option', { name: /^ZSortMail/ }).click();
});

test('the language is chosen in Settings, changes the menus at once, and is remembered', async ({
  page,
}) => {
  await page.goto(url('/settings'));
  await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible();
  await page.getByRole('combobox', { name: 'Language' }).click();
  await page.getByRole('option', { name: 'Deutsch', exact: true }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Einstellungen' })).toBeVisible();
  await expect(
    page.getByRole('navigation', { name: 'Main' }).getByRole('link', { name: 'Heute' }),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'de');
  await page.screenshot({ path: 'test-results/screens/german.png' });
  await page.reload();
  await expect(page.getByRole('heading', { level: 1, name: 'Einstellungen' })).toBeVisible();
  await page.getByRole('combobox', { name: 'Sprache' }).click();
  await page.getByRole('option', { name: 'English', exact: true }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible();
});

test('a dropdown stays attached to its button when the page scrolls, and a switch moves cleanly', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1360, height: 700 });
  await page.goto(url('/settings'));
  const field = page.getByRole('combobox', { name: 'Keys' });
  await field.scrollIntoViewIfNeeded();
  await field.click();
  const list = page.getByRole('listbox', { name: 'Keys' });
  await expect(list).toBeVisible();
  const gap = async () => {
    const [b, l] = await Promise.all([field.boundingBox(), list.boundingBox()]);
    if (!b || !l) throw new Error('missing');
    return Math.round(Math.min(Math.abs(l.y - (b.y + b.height)), Math.abs(l.y + l.height - b.y)));
  };
  const before = await gap();
  await page.mouse.move(900, 300);
  await page.mouse.wheel(0, 60);
  await page.waitForTimeout(150);
  expect(await gap()).toBeLessThanOrEqual(before + 2);
  await page.keyboard.press('Escape');

  const sw = page.getByRole('switch', { name: 'Tidy up letters' });
  await sw.scrollIntoViewIfNeeded();
  const knob = sw.locator('xpath=../span[contains(@class,"toggle-knob")]');
  const track = sw.locator('xpath=..');
  const t = await track.boundingBox();
  const first = await knob.boundingBox();
  await sw.click();
  await page.waitForTimeout(400);
  const second = await knob.boundingBox();
  if (!t || !first || !second) throw new Error('no switch');
  expect(Math.abs(second.x - first.x)).toBeGreaterThanOrEqual(19);
  expect(Math.abs(second.x - first.x)).toBeLessThanOrEqual(21);
  for (const k of [first, second]) {
    expect(k.x).toBeGreaterThanOrEqual(t.x);
    expect(k.x + k.width).toBeLessThanOrEqual(t.x + t.width);
    expect(k.height).toBeCloseTo(20, 0);
  }
  await sw.click();

  await page.getByRole('button', { name: 'Try it' }).scrollIntoViewIfNeeded();
  const tryIt = await page.getByRole('button', { name: 'Try it' }).boundingBox();
  expect(tryIt?.height).toBeLessThan(40);
});

test('a custom theme is made from hex or rgb colours, shown live, checked for readability, saved and removed', async ({
  page,
}) => {
  await page.goto(url('/settings'));
  await page.getByRole('button', { name: 'Create a theme' }).click();
  const dialog = page.getByRole('dialog', { name: 'New theme' });
  await dialog.getByLabel('Name').fill('Dusk');

  await dialog.getByRole('textbox', { name: 'Accent as hex or rgb', exact: true }).fill('#ff7800');
  await expect
    .poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--blue')))
    .toBe('#ff7800');
  await dialog
    .getByRole('textbox', { name: 'Background as hex or rgb', exact: true })
    .fill('rgb(10, 20, 40)');
  await expect
    .poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--desk')))
    .toBe('#0a1428');

  await dialog.getByRole('textbox', { name: 'Text as hex or rgb', exact: true }).fill('banana');
  await expect(dialog.getByRole('alert').filter({ hasText: 'Text: not a colour' })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Save theme' })).toBeDisabled();

  await dialog.getByRole('textbox', { name: 'Text as hex or rgb', exact: true }).fill('#0b1428');
  await expect(dialog.getByText('Text on the background', { exact: true })).toBeVisible();
  await expect(
    dialog.getByText(/combinations are hard to read|One combination is hard to read/),
  ).toBeVisible();
  await dialog.getByRole('textbox', { name: 'Text as hex or rgb', exact: true }).fill('#e5ecf3');
  await page.screenshot({ path: 'test-results/screens/theme-editor.png' });

  const text = await dialog.getByLabel('Theme as text').inputValue();
  expect(text).toContain('"accent"'.replace('accent', 'blue'));
  expect(text).toContain('#ff7800');

  await dialog.getByRole('button', { name: 'Save theme' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-custom-theme', 'dark');
  await expect(page.getByRole('radio', { name: /Dusk/ })).toBeChecked();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-custom-theme', 'dark');
  expect(await page.evaluate(() => document.documentElement.style.getPropertyValue('--blue'))).toBe(
    '#ff7800',
  );

  await page.getByRole('button', { name: 'Edit the theme Dusk' }).click();
  const edit = page.getByRole('dialog', { name: 'Edit Dusk' });
  await edit.getByRole('textbox', { name: 'Accent as hex or rgb', exact: true }).fill('#00ff00');
  await expect
    .poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--blue')))
    .toBe('#00ff00');
  await edit.getByRole('button', { name: 'Cancel' }).click();
  await expect
    .poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--blue')))
    .toBe('#ff7800');

  await page.getByRole('button', { name: 'Delete the theme Dusk' }).click();
  await expect(page.getByRole('radio', { name: /Dusk/ })).toHaveCount(0);
  await expect(page.locator('html')).not.toHaveAttribute('data-custom-theme', /.+/);
  expect(await page.evaluate(() => document.documentElement.style.getPropertyValue('--blue'))).toBe(
    '',
  );
});
