import { MagnifyingGlassMinus, MagnifyingGlassPlus } from '@phosphor-icons/react';
import type { PDFDocumentProxy } from 'pdfjs-dist';
import { useEffect, useRef, useState } from 'react';
import { IconButton } from '@/ui/controls';

export default function PdfView({ src, name }: { src: string; name: string }) {
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const [error, setError] = useState(false);
  const [zoom, setZoom] = useState(1);
  const [page, setPage] = useState(1);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let live = true;
    let loaded: PDFDocumentProxy | null = null;
    (async () => {
      try {
        const [pdfjs, worker] = await Promise.all([
          import('pdfjs-dist'),
          import('pdfjs-dist/build/pdf.worker.min.mjs?url'),
        ]);
        pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
        const data = new Uint8Array(await (await fetch(src)).arrayBuffer());
        loaded = await pdfjs.getDocument({ data, isEvalSupported: false }).promise;
        if (live) setDoc(loaded);
      } catch {
        if (live) setError(true);
      }
    })();
    return () => {
      live = false;
      void loaded?.destroy();
    };
  }, [src]);

  if (error) {
    return (
      <p className="text-ui text-text-2">
        This PDF could not be shown. Save it to open it elsewhere.
      </p>
    );
  }
  if (!doc) return <p className="text-ui text-text-2">Opening the PDF</p>;

  return (
    <div className="flex h-[68vh] w-full flex-col">
      <div className="mb-2 flex items-center justify-center gap-2 text-ui text-text-2">
        <IconButton
          label="Zoom out"
          disabled={zoom <= 0.5}
          onClick={() => setZoom((z) => Math.max(0.5, z - 0.25))}
        >
          <MagnifyingGlassMinus size={17} />
        </IconButton>
        <span className="tabular min-w-24 text-center" aria-live="polite">
          Page {page} of {doc.numPages}, {Math.round(zoom * 100)}%
        </span>
        <IconButton
          label="Zoom in"
          disabled={zoom >= 3}
          onClick={() => setZoom((z) => Math.min(3, z + 0.25))}
        >
          <MagnifyingGlassPlus size={17} />
        </IconButton>
      </div>
      <div ref={box} title={name} className="scroll-y min-h-0 flex-1 rounded-sheet bg-desk">
        <div className="mx-auto flex w-fit flex-col gap-3 p-3">
          {Array.from({ length: doc.numPages }, (_, i) => i + 1).map((n) => (
            <Page key={n} doc={doc} number={n} zoom={zoom} root={box} onVisible={setPage} />
          ))}
        </div>
      </div>
    </div>
  );
}

function Page({
  doc,
  number,
  zoom,
  root,
  onVisible,
}: {
  doc: PDFDocumentProxy;
  number: number;
  zoom: number;
  root: React.RefObject<HTMLDivElement | null>;
  onVisible: (n: number) => void;
}) {
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 600 * zoom, h: 800 * zoom });
  const [seen, setSeen] = useState(number === 1);

  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry) return;
        if (entry.isIntersecting) setSeen(true);
        if (entry.intersectionRatio > 0.5) onVisible(number);
      },
      { root: root.current, threshold: [0, 0.5], rootMargin: '300px 0px' },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [number, onVisible, root]);

  useEffect(() => {
    if (!seen) return;
    let cancelled = false;
    let task: { cancel: () => void; promise: Promise<unknown> } | null = null;
    void doc.getPage(number).then((page) => {
      if (cancelled) return;
      const base = page.getViewport({ scale: 1 });
      const fit = Math.min(1, ((root.current?.clientWidth ?? 640) - 40) / base.width);
      const viewport = page.getViewport({ scale: fit * zoom });
      const ratio = window.devicePixelRatio || 1;
      const el = canvas.current;
      const context = el?.getContext('2d');
      if (!el || !context) return;
      el.width = Math.floor(viewport.width * ratio);
      el.height = Math.floor(viewport.height * ratio);
      setSize({ w: viewport.width, h: viewport.height });
      task = page.render({
        canvas: el,
        canvasContext: context,
        viewport,
        transform: ratio !== 1 ? [ratio, 0, 0, ratio, 0, 0] : undefined,
      });
      task.promise.catch(() => {});
    });
    return () => {
      cancelled = true;
      task?.cancel();
    };
  }, [doc, number, zoom, seen, root]);

  return (
    <div ref={wrap} style={{ width: size.w, height: size.h }} className="bg-paper shadow-sheet">
      <canvas
        ref={canvas}
        style={{ width: size.w, height: size.h }}
        aria-label={`Page ${number}`}
        role="img"
      />
    </div>
  );
}
