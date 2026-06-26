import { useCallback, useEffect, useState } from 'react';
import { eqGetBands, eqSetBand, eqSetPreamp, eqReset, surroundGetWidth, surroundSetWidth } from '@/utils/ipc';
import styles from './Equalizer.module.css';

/// ISO 10-band center frequencies (Hz)
const FREQ_LABELS = ['31', '62', '125', '250', '500', '1K', '2K', '4K', '8K', '16K'];

/// Presets: name → [10 band gains in dB, preamp dB]
const PRESETS: Record<string, { bands: number[]; preamp: number }> = {
  Flat:     { bands: [ 0,  0,  0,  0,  0,  0,  0,  0,  0,  0], preamp: 0 },
  Rock:     { bands: [ 5,  4, -2, -3, -1,  2,  5,  6,  5,  4], preamp: -2 },
  Pop:      { bands: [-1,  2,  3,  1, -2, -1,  1,  2,  3,  2], preamp: -1 },
  Classical:{ bands: [ 3,  2,  1,  0, -1, -1,  0,  1,  2,  3], preamp:  0 },
  Jazz:     { bands: [ 4,  2,  0,  1, -1, -1,  0,  2,  3,  3], preamp: -1 },
  HipHop:   { bands: [ 5,  4,  0,  2, -2, -1,  1,  0,  2,  3], preamp: -2 },
  Vocal:    { bands: [-3, -2,  0,  2,  5,  4,  2,  1,  0, -1], preamp:  0 },
  Bass:     { bands: [ 6,  5,  3,  1,  0, -1, -2, -2, -2, -2], preamp: -3 },
  Treble:   { bands: [-3, -2, -1,  0,  1,  2,  4,  5,  6,  6], preamp: -1 },
  Custom:   { bands: [ 0,  0,  0,  0,  0,  0,  0,  0,  0,  0], preamp: 0 },
};

export default function Equalizer() {
  const [bands, setBands] = useState<number[]>(Array(10).fill(0));
  const [preamp, setPreampState] = useState(0);
  const [activePreset, setActivePreset] = useState('Custom');
  const [surround, setSurroundState] = useState(0);

  // Load current EQ state from backend on mount
  useEffect(() => {
    (async () => {
      try {
        const b = await eqGetBands();
        if (b?.length === 10) setBands(b);
        const sw = await surroundGetWidth();
        setSurroundState(sw);
      } catch (e) { console.warn('[TTPlayer] EQ init:', e); }
    })();
  }, []);

  const applyPreset = useCallback(async (name: string) => {
    const p = PRESETS[name];
    if (!p) return;
    setActivePreset(name);
    setBands([...p.bands]);
    setPreampState(p.preamp);
    // Push to backend
    await eqSetPreamp(p.preamp);
    for (let i = 0; i < 10; i++) {
      await eqSetBand(i, p.bands[i]);
    }
  }, []);

  const handleBandChange = useCallback(async (index: number, db: number) => {
    const next = [...bands];
    next[index] = db;
    setBands(next);
    setActivePreset('Custom');
    await eqSetBand(index, db);
  }, [bands]);

  const handlePreampChange = useCallback(async (db: number) => {
    setPreampState(db);
    setActivePreset('Custom');
    await eqSetPreamp(db);
  }, []);

  const handleReset = useCallback(async () => {
    setBands(Array(10).fill(0));
    setPreampState(0);
    setActivePreset('Flat');
    await eqReset();
  }, []);

  const handleSurroundChange = useCallback(async (val: number) => {
    setSurroundState(val);
    await surroundSetWidth(val);
  }, []);

  return (
    <div className={styles.eq}>
      <div className={styles.header}>
        <span className={styles.title}>🎛 均衡器</span>
        <button className={styles.resetBtn} onClick={handleReset} title="归零">⟲</button>
      </div>

      {/* Sliders row */}
      <div className={styles.sliders}>
        {/* Preamp */}
        <div className={styles.sliderCol}>
          <input
            type="range"
            className={styles.slider}
            {...{ orient: 'vertical' } as any}
            min={-12}
            max={12}
            step={0.5}
            value={preamp}
            onChange={(e) => handlePreampChange(parseFloat(e.target.value))}
          />
          <span className={styles.freqLabel}>PRE</span>
          <span className={styles.dbLabel}>{preamp > 0 ? '+' : ''}{preamp.toFixed(1)}</span>
        </div>

        {FREQ_LABELS.map((label, i) => (
          <div key={label} className={styles.sliderCol}>
            <input
              type="range"
              className={styles.slider}
              {...{ orient: 'vertical' } as any}
              min={-12}
              max={12}
              step={0.5}
              value={bands[i]}
              onChange={(e) => handleBandChange(i, parseFloat(e.target.value))}
            />
            <span className={styles.freqLabel}>{label}</span>
            <span className={styles.dbLabel}>{bands[i] > 0 ? '+' : ''}{bands[i].toFixed(1)}</span>
          </div>
        ))}
      </div>

      {/* Preset buttons */}
      <div className={styles.presets}>
        {Object.keys(PRESETS).filter(k => k !== 'Custom').map(name => (
          <button
            key={name}
            className={`${styles.presetBtn} ${activePreset === name ? styles.presetActive : ''}`}
            onClick={() => applyPreset(name)}
          >
            {name}
          </button>
        ))}
      </div>

      {/* Surround slider */}
      <div className={styles.surroundRow}>
        <span className={styles.surroundLabel}>🔊 环绕声</span>
        <input
          type="range"
          className={styles.surroundSlider}
          min={0}
          max={10}
          step={1}
          value={surround}
          onChange={(e) => handleSurroundChange(parseInt(e.target.value, 10))}
        />
        <span className={styles.surroundValue}>{surround}</span>
      </div>
    </div>
  );
}
