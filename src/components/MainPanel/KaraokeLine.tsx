import type { LrcLine } from '@/utils/ipc';
import styles from './LyricsPanel.module.css';

/**
 * Karaoke line with word-level sung/unsung highlighting.
 *
 * The sung threshold is derived from each word's timestamp relative to the
 * line's start; `progress` is the store-provided line progress in [0,1].
 * Pure render component — no state, no effects.
 */
export function KaraokeLine({
  words,
  progress,
  lineTimeMs,
}: {
  words: NonNullable<LrcLine['words']>;
  progress: number;
  lineTimeMs: number;
}) {
  const lineDuration = words.length > 1
    ? words[words.length - 1].timeMs - lineTimeMs
    : 5000;

  return (
    <span className={styles.karaokeLine}>
      {words.map((word, i) => {
        const wordProgress = lineDuration > 0
          ? (word.timeMs - lineTimeMs) / lineDuration
          : i / Math.max(words.length - 1, 1);
        const isSung = progress >= wordProgress;
        return (
          <span key={i} className={`${styles.word} ${isSung ? styles.wordSung : ''}`}>
            {word.text}
          </span>
        );
      })}
    </span>
  );
}
