import { useState, useEffect } from 'react';
import { usePlayerStore } from '@/stores/player';

/**
 * Ticker cycling through title / artist / album / audio-info every 3 seconds.
 */
export function useTicker() {
  const currentFile = usePlayerStore((s) => s.currentFile);
  const metadata = usePlayerStore((s) => s.metadata);
  const [tickerIndex, setTickerIndex] = useState(0);

  useEffect(() => {
    const id = window.setInterval(() => setTickerIndex((p) => p + 1), 3000);
    return () => clearInterval(id);
  }, []);

  const fileName = currentFile ? (currentFile.split(/[/\\]/).pop() ?? currentFile) : null;
  const displayTitle = metadata?.title || fileName || 'TTPlayer-Next';
  const displayArtist = metadata?.artist || '';
  const displayAlbum = metadata?.album || '';
  const audioInfo = (() => {
    const parts: string[] = [];
    if (metadata?.sampleRate) parts.push(`${metadata.sampleRate / 1000}kHz`);
    if (metadata?.bitDepth) parts.push(`${metadata.bitDepth}bit`);
    if (metadata?.channels) parts.push(metadata.channels === 2 ? 'Stereo' : metadata.channels === 1 ? 'Mono' : `${metadata.channels}ch`);
    if (metadata?.bitRate) parts.push(`${metadata.bitRate}kbps`);
    return parts.join(' · ');
  })();

  const lines: string[] = [displayTitle];
  if (displayArtist) lines.push(displayArtist);
  if (displayAlbum) lines.push(displayAlbum);
  if (audioInfo) lines.push(audioInfo);

  return { tickerIndex, currentTicker: lines[tickerIndex % lines.length] };
}
