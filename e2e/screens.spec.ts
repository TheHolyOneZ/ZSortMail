import AxeBuilder from '@axe-core/playwright';
import { expect, type Page, test } from '@playwright/test';

const NOW = '2026-11-18T10:00:00+01:00';

const SIZES = {
  wide: { width: 1360, height: 860 },
  narrow: { width: 900, height: 780 },
  small: { width: 720, height: 600 },
} as const;
const THEMES = ['light', 'dark'] as const;

async function personId(page: Page, name: string): Promise<number> {
  const response = await page.request.post('/ipc/list_people', {
    data: { filter: 'all', query: name },
  });
  const people = (await response.json()) as { id: number }[];
  if (!people[0]) throw new Error(`no demo person matches ${name}`);
  return people[0].id;
}

function url(hash: string, extra = ''): string {
  return `/?now=${encodeURIComponent(NOW)}${extra}#${hash}`;
}

function watchForErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  return errors;
}

async function settle(page: Page) {
  await expect(page.locator('#root')).not.toBeEmpty();
  await page.waitForTimeout(900);
}

for (const theme of THEMES) {
  for (const [name, size] of Object.entries(SIZES)) {
    test(`strand ${name} ${theme}`, async ({ browser }) => {
      const context = await browser.newContext({ viewport: size, colorScheme: theme });
      const page = await context.newPage();
      const errors = watchForErrors(page);
      await page.goto(url('/people/1/c/1'));
      await settle(page);
      await page.screenshot({ path: `test-results/screens/strand-${name}-${theme}.png` });
      expect(errors).toEqual([]);
      await context.close();
    });
  }

  test(`people list narrow ${theme}`, async ({ browser }) => {
    const context = await browser.newContext({ viewport: SIZES.narrow, colorScheme: theme });
    const page = await context.newPage();
    const errors = watchForErrors(page);
    await page.goto(url('/people'));
    await settle(page);
    await page.screenshot({ path: `test-results/screens/people-narrow-${theme}.png` });
    expect(errors).toEqual([]);
    await context.close();
  });

  test(`design specimen ${theme}`, async ({ browser }) => {
    const context = await browser.newContext({ viewport: SIZES.wide, colorScheme: theme });
    const page = await context.newPage();
    const errors = watchForErrors(page);
    await page.goto(url('/__design'));
    await settle(page);
    await page.screenshot({
      path: `test-results/screens/specimen-${theme}.png`,
      fullPage: false,
    });
    await page.evaluate(() => {
      const el = document.querySelector<HTMLElement>('[data-testid="design-specimen"]');
      if (el) el.style.height = `${el.scrollHeight}px`;
      document.body.style.overflow = 'visible';
    });
    await page.setViewportSize({ width: SIZES.wide.width, height: 3000 });
    await page.screenshot({ path: `test-results/screens/specimen-full-${theme}.png` });
    expect(errors).toEqual([]);
    await context.close();
  });
}

test('settings renders and switches theme', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.setViewportSize(SIZES.wide);
  await page.goto(url('/settings'));
  await settle(page);
  await page.getByRole('radio', { name: /^Ink/ }).check({ force: true });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('radio', { name: 'Compact' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await page.screenshot({ path: 'test-results/screens/settings-dark.png' });
  expect(errors).toEqual([]);
});

test('opening a person in a wide window opens their latest conversation', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.setViewportSize(SIZES.wide);
  await page.goto(url('/people'));
  await settle(page);
  const dad = await personId(page, 'Dad');
  await page.getByRole('option', { name: /Dad/ }).click();
  await expect(page).toHaveURL(new RegExp(`#/people/${dad}/c/\\d+`));
  await expect(page.getByRole('heading', { level: 2, name: 'dinner sat?' })).toBeVisible();
  expect(errors).toEqual([]);
});

test('search narrows the people list and accents are ignored', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.setViewportSize(SIZES.wide);
  await page.goto(url('/people'));
  await settle(page);
  await page.getByRole('searchbox', { name: 'Search people' }).fill('amelie');
  await expect(page.getByRole('option')).toHaveCount(1);
  await expect(page.getByRole('option', { name: /Amélie/ })).toBeVisible();
  expect(errors).toEqual([]);
});

test('a reply is appended to the conversation', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.setViewportSize(SIZES.wide);
  await page.goto(url('/people/1/c/1'));
  await settle(page);
  const before = await page.locator('[data-knot]').count();
  await page.getByPlaceholder(/Reply to/).fill('Thanks, looking now.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.locator('[data-knot]')).toHaveCount(before + 1);
  await expect(
    page
      .getByRole('region', { name: 'Conversation', exact: true })
      .getByText('Thanks, looking now.'),
  ).toBeVisible();
  await expect(
    page.getByRole('region', { name: 'Conversations' }).getByText('Thanks, looking now.'),
  ).toBeVisible();
  expect(errors).toEqual([]);
});

test('the 300-message conversation opens quickly and stays usable', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.setViewportSize(SIZES.wide);
  await page.goto(url(`/people/${await personId(page, 'Whitaker')}`));
  await settle(page);
  const started = Date.now();
  await page.getByRole('button', { name: /Re: Re: Re: Fwd: team standup notes/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: /team standup notes/ })).toBeVisible();
  const elapsed = Date.now() - started;
  console.log(`300-message conversation visible in ${elapsed} ms`);
  expect(elapsed).toBeLessThan(2000);
  expect(errors).toEqual([]);
});

const AUDITED = [
  '/people/1/c/1',
  '/settings',
  '/__design',
  '/today',
  '/river',
  '/people',
  '/new-senders',
  '/waiting',
  '/routines',
  '/rules',
  '/drafts',
  '/hidden',
  '/notifications',
];

for (const route of AUDITED) {
  test(`accessibility ${route}`, async ({ page }) => {
    await page.setViewportSize(SIZES.wide);
    await page.goto(url(route));
    await settle(page);
    const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa']).analyze();
    const summary = results.violations.map(
      (v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(' ')).join(' | ')}`,
    );
    expect(summary).toEqual([]);
  });

  test(`accessibility ${route}, dark`, async ({ browser }) => {
    const context = await browser.newContext({ viewport: SIZES.wide, colorScheme: 'dark' });
    const page = await context.newPage();
    await page.goto(url(route));
    await settle(page);
    const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa']).analyze();
    const summary = results.violations.map(
      (v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(' ')).join(' | ')}`,
    );
    expect(summary).toEqual([]);
    await context.close();
  });
}

test('narrow windows fold between the three levels and back', async ({ browser }) => {
  const context = await browser.newContext({ viewport: SIZES.narrow });
  await context.addInitScript(() =>
    localStorage.setItem(
      'zsm-ui',
      JSON.stringify({
        state: { theme: 'system', density: 'comfortable', motion: 'full', categoriesFirst: false },
        version: 0,
      }),
    ),
  );
  const page = await context.newPage();
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await settle(page);

  await page.getByRole('option', { name: /Maya/ }).click();
  await expect(page).toHaveURL(/#\/people\/1$/);
  await expect(page.getByRole('button', { name: /Q3 launch checklist/ })).toBeVisible();

  await page.getByRole('button', { name: /Q3 launch checklist/ }).click();
  await expect(page).toHaveURL(/#\/people\/1\/c\/\d+$/);
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();

  await page.getByRole('button', { name: 'Back to conversations' }).click();
  await expect(page).toHaveURL(/#\/people\/1$/);
  await page.getByRole('button', { name: 'Back to people' }).click();
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const pane = document.querySelector('section[aria-label="Conversations"]')?.parentElement;
          return pane ? getComputedStyle(pane).transform : 'gone';
        }),
      { timeout: 1500, intervals: [20] },
    )
    .toMatch(/^matrix3d/);
  await expect(page).toHaveURL(/#\/people$/);

  await expect(page.locator('section[aria-label="Conversations"]')).toHaveCount(0);
  await expect(page.locator('section[aria-label="Conversation"]')).toHaveCount(0);
  await expect(page.locator('section[aria-label="People"]')).toHaveCount(1);
  expect(errors).toEqual([]);
  await context.close();
});

test('arrow keys move through the people list', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.setViewportSize(SIZES.wide);
  const maya = await personId(page, 'Okafor');
  const dad = await personId(page, 'Dad');
  await page.goto(url(`/people/${maya}`));
  await settle(page);
  await page.getByRole('listbox', { name: 'People' }).focus();
  await page.keyboard.press('ArrowDown');
  await expect(page).toHaveURL(new RegExp(`#/people/${dad}`));
  await page.keyboard.press('ArrowUp');
  await expect(page).toHaveURL(new RegExp(`#/people/${maya}(/|$)`));
  expect(errors).toEqual([]);
});
