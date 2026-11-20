const SAMPLES = [
  'Łukasz Đức Anh Nguyễn Åsa Öberg',
  'Алексей Смирнов',
  'Ελένη Παπά',
  '田中 弘 佐藤 美咲 李 明',
  '김 민준',
  'علي حسن محمد',
  'שרה לוי נועה כץ',
  'สมชาย',
  'अनिल शर्मा',
  '🎉 👩‍💻',
];

export function warmFallbackFonts(): void {
  if (typeof document === 'undefined') return;
  const run = () => {
    const probe = document.createElement('div');
    probe.setAttribute('aria-hidden', 'true');
    probe.style.cssText =
      'position:fixed;left:-9999px;top:0;visibility:hidden;pointer-events:none;' +
      'white-space:nowrap;font-family:var(--font-ui);font-size:15px';
    probe.textContent = SAMPLES.join('   ');
    document.body.append(probe);
    void probe.offsetWidth;
    requestAnimationFrame(() => probe.remove());
  };
  if (typeof window.requestIdleCallback === 'function') {
    window.requestIdleCallback(run, { timeout: 2000 });
  } else {
    window.setTimeout(run, 400);
  }
}
