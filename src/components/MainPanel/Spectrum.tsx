import { useEffect, useRef, useState } from 'react';
import { usePlayerStore } from '@/stores/player';
import styles from './Spectrum.module.css';

const BAR_COUNT = 32;
const TRANSITION_MS = 120;

/** Resolve any CSS color expression (hex, rgb, var()) to [r, g, b]. */
function resolveColor(expr: string): [number, number, number] {
  const el = document.createElement('div');
  el.style.color = expr;
  el.style.display = 'none';
  document.body.appendChild(el);
  const computed = getComputedStyle(el).color;
  document.body.removeChild(el);
  const m = computed.match(/(\d+(?:\.\d+)?)/g);
  if (m && m.length >= 3) {
    return [parseFloat(m[0]), parseFloat(m[1]), parseFloat(m[2])];
  }
  return [124, 108, 240];
}

function lerp(a: number, b: number, t: number) {
  return Math.round(a + (b - a) * t);
}

export default function Spectrum() {
  const [heights, setHeights] = useState<number[]>(new Array(BAR_COUNT).fill(0));
  const prevRef = useRef<Float32Array>(new Float32Array(BAR_COUNT));

  // Resolve spectrum colours from skin CSS variables once per mount.
  // Falls back to --accent / --accent-light when a skin doesn't define them.
  const colors = useRef<{ top: [number, number, number]; bottom: [number, number, number] }>({
    top: [124, 108, 240],
    bottom: [196, 181, 253],
  });
  useEffect(() => {
    colors.current = {
      top: resolveColor('var(--spectrum-top, var(--accent-light, #C4B5FD))'),
      bottom: resolveColor('var(--spectrum-bottom, var(--accent, #7c3aed))'),
    };
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => {
      const state = usePlayerStore.getState();
      const src = state.spectrum;
      const srcLen = src.length;
      const prev = prevRef.current;
      const next = new Array(BAR_COUNT);

      if (state.state !== 'Playing') {
        let anyActive = false;
        for (let i = 0; i < BAR_COUNT; i++) {
          const val = prev[i] * 0.6;
          prev[i] = val;
          next[i] = val > 0.01 ? Math.round(val * 100) : 0;
          if (next[i] > 0) anyActive = true;
        }
        if (!anyActive) return;
      } else {
        for (let i = 0; i < BAR_COUNT; i++) {
          const t = i / BAR_COUNT;
          const srcIdx = Math.min(srcLen - 1, Math.floor(t * t * srcLen));
          const raw = src[srcIdx] || 0;
          const db = 20 * Math.log10(Math.max(raw, 1e-6));
          const normalized = Math.max(0, (db + 60) / 60);
          const prevVal = prev[i];
          const val = normalized > prevVal ? normalized : prevVal * 0.78;
          prev[i] = val;
          next[i] = Math.round(val * 100);
        }
      }

      setHeights(next);
    }, 60);

    return () => clearInterval(id);
  }, []);

  const { top, bottom } = colors.current;

  return (
    <div className={styles.container}>
      {heights.map((h, i) => {
        const t = i / (BAR_COUNT - 1);
        const r = lerp(bottom[0], top[0], t);
        const g = lerp(bottom[1], top[1], t);
        const b = lerp(bottom[2], top[2], t);
        return (
          <div
            key={i}
            className={styles.bar}
            style={{
              height: `${h}%`,
              backgroundColor: `rgb(${r},${g},${b})`,
              transition: `height ${TRANSITION_MS}ms ease-out`,
            }}
          />
        );
      })}
    </div>
  );
}
