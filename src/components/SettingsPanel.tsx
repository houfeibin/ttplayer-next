import { useState, useEffect, useCallback } from 'react';
import { emitTo, listen } from '@tauri-apps/api/event';
import { usePlayerStore } from '@/stores/player';
import { useSkinStore } from '@/stores/skin';
import { useLyricsStore, type LyricsTextAlign } from '@/stores/lyrics';
import { applySkin } from '@/components/SkinProvider';
import {
  crossfadeGetDuration, crossfadeSetDuration,
  setVolume as ipcSetVolume,
  lyricsGetToken, lyricsSetToken, lyricsHasToken,
  themeGetMode, themeSetMode,
  desktopLyricsGet, desktopLyricsSet, desktopLyricsReset,
  DESKTOP_LYRICS_FONT_MIN, DESKTOP_LYRICS_FONT_MAX,
  DESKTOP_LYRICS_OPACITY_MIN, DESKTOP_LYRICS_OPACITY_MAX, DESKTOP_LYRICS_OPACITY_DEFAULT,
  type DesktopLyricsSettings,
} from '@/utils/ipc';
import { logWarn } from '@/utils/logger';
import styles from './SettingsPanel.module.css';

interface Props {
  onClose: () => void;
}

/** 系统可用字体选项（值即 CSS font-family 字符串）。 */
const FONT_FAMILY_OPTIONS: { label: string; value: string }[] = [
  { label: '系统默认', value: 'system-ui, sans-serif' },
  { label: '微软雅黑', value: '"Microsoft YaHei", sans-serif' },
  { label: '黑体', value: '"SimHei", sans-serif' },
  { label: '宋体', value: '"SimSun", serif' },
  { label: '楷体', value: '"KaiTi", serif' },
  { label: '仿宋', value: '"FangSong", serif' },
  { label: 'Arial', value: 'Arial, sans-serif' },
  { label: 'Times New Roman', value: '"Times New Roman", serif' },
  { label: 'Georgia', value: 'Georgia, serif' },
  { label: 'Courier New', value: '"Courier New", monospace' },
];

/** 字体颜色预设方案。 */
const FONT_COLOR_PRESETS = [
  '#a78bfa', '#60a5fa', '#f87171', '#34d399',
  '#fbbf24', '#f472b6', '#ffffff', '#22d3ee',
];

/** 主界面歌词文字对齐方式选项。 */
const LYRICS_TEXT_ALIGN_OPTIONS: { label: string; value: LyricsTextAlign }[] = [
  { label: '左对齐', value: 'left' },
  { label: '居中', value: 'center' },
  { label: '右对齐', value: 'right' },
];

export default function SettingsPanel({ onClose }: Props) {
  const volume = usePlayerStore((s) => s.volume);
  const setVolume = usePlayerStore((s) => s.setVolume);
  const currentSkinId = useSkinStore((s) => s.currentSkinId);
  const availableSkins = useSkinStore((s) => s.availableSkins);

  const [crossfadeMs, setCrossfadeMs] = useState(3000);
  const [themeMode, setThemeMode] = useState('dark');
  const [activeTab, setActiveTab] = useState<'audio' | 'lyrics' | 'skin' | 'about'>('audio');

  // Lyrics API token state
  const [apiToken, setApiToken] = useState('');
  const [apiTokenMasked, setApiTokenMasked] = useState('');
  const [tokenSaving, setTokenSaving] = useState(false);
  const [tokenError, setTokenError] = useState('');
  const [tokenSuccess, setTokenSuccess] = useState('');

  // Desktop lyrics settings state (font family / size / style / color / lock / karaoke / display mode)
  const [desktopSettings, setDesktopSettings] = useState<DesktopLyricsSettings>({
    font_size: 28, locked: false, font_family: 'system-ui, sans-serif',
    bold: true, italic: false, font_color: '#a78bfa',
    karaoke: true, line_count: 1, direction: 'horizontal',
    opacity: DESKTOP_LYRICS_OPACITY_DEFAULT,
  });

  // 主界面歌词样式（来自 lyrics store，持久化到 localStorage）
  const mainLyricsFontFamily = useLyricsStore((s) => s.fontFamily);
  const mainLyricsTextAlign = useLyricsStore((s) => s.textAlign);
  const mainLyricsFontSize = useLyricsStore((s) => s.fontSize);
  const mainLyricsLineHeight = useLyricsStore((s) => s.lineHeight);
  const setMainLyricsFontFamily = useLyricsStore((s) => s.setFontFamily);
  const setMainLyricsTextAlign = useLyricsStore((s) => s.setTextAlign);
  const setMainLyricsFontSize = useLyricsStore((s) => s.setFontSize);
  const setMainLyricsLineHeight = useLyricsStore((s) => s.setLineHeight);

  // 系统已安装字体列表（通过浏览器 queryLocalFonts API 枚举，失败时回退到精选列表）
  const [systemFonts, setSystemFonts] = useState<{ label: string; value: string }[]>([]);
  const [fontsLoading, setFontsLoading] = useState(false);
  const [fontsLoadError, setFontsLoadError] = useState('');

  /**
   * 通过浏览器 `queryLocalFonts()` API 枚举系统已安装字体。
   * - Chrome 103+ / WebView2 支持，需要用户授权（首次调用弹出权限提示）
   * - 失败时回退到 `FONT_FAMILY_OPTIONS` 精选列表，保证功能可用
   * - 兼容 TrueType / OpenType 等常见字体格式（由系统字体渲染处理）
   */
  const loadSystemFonts = useCallback(async () => {
    setFontsLoading(true);
    setFontsLoadError('');
    try {
      const w = window as unknown as { queryLocalFonts?: () => Promise<Array<{ family: string }>> };
      if (typeof w.queryLocalFonts !== 'function') {
        setFontsLoadError('当前环境不支持系统字体枚举，已显示精选字体列表');
        return;
      }
      const fonts = await w.queryLocalFonts();
      // 按 family 去重，构造 CSS font-family 字符串（含 sans-serif 回退）
      const familyMap = new Map<string, string>();
      for (const f of fonts) {
        if (f.family && !familyMap.has(f.family)) {
          familyMap.set(f.family, `"${f.family}", sans-serif`);
        }
      }
      const opts = Array.from(familyMap.entries())
        .map(([family, value]) => ({ label: family, value }))
        .sort((a, b) => a.label.localeCompare(b.label, 'zh-Hans'));
      if (opts.length > 0) {
        setSystemFonts(opts);
      } else {
        setFontsLoadError('未检测到系统字体，已显示精选字体列表');
      }
    } catch (e: any) {
      logWarn('queryLocalFonts', e);
      setFontsLoadError('系统字体加载失败，已显示精选字体列表');
    } finally {
      setFontsLoading(false);
    }
  }, []);

  // 首次进入歌词标签页时尝试加载系统字体（失败自动回退，不阻塞 UI）
  useEffect(() => {
    if (activeTab === 'lyrics' && systemFonts.length === 0 && !fontsLoading && !fontsLoadError) {
      void loadSystemFonts();
    }
  }, [activeTab, systemFonts.length, fontsLoading, fontsLoadError, loadSystemFonts]);

  useEffect(() => {
    crossfadeGetDuration().then(ms => setCrossfadeMs(ms)).catch(e => logWarn('crossfadeGetDuration', e));
  }, []);

  useEffect(() => {
    lyricsGetToken().then((token) => {
      if (token) {
        setApiToken(token);
        setApiTokenMasked(token.length > 8
          ? `${token.slice(0, 4)}****${token.slice(-4)}`
          : '****');
      }
    }).catch(e => logWarn('lyricsGetToken', e));
  }, []);

  useEffect(() => {
    themeGetMode().then(setThemeMode).catch(() => setThemeMode('dark'));
  }, []);

  // 加载桌面歌词设置并监听变更（与桌面歌词窗口双向同步）
  useEffect(() => {
    desktopLyricsGet().then(setDesktopSettings).catch((e) => logWarn('desktopLyricsGet', e));
    const unlisten = listen<DesktopLyricsSettings>('desktop-lyrics-settings-changed', (event) => {
      setDesktopSettings(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // 通用更新：乐观改本地 + 持久化（后端会广播回来以服务端为准）
  const updateDesktop = async (patch: Partial<DesktopLyricsSettings>) => {
    setDesktopSettings((prev) => ({ ...prev, ...patch }));
    await desktopLyricsSet(patch).catch((e) => logWarn('desktopLyricsSet', e));
  };

  const handleDesktopFontSize = (val: number) => {
    const clamped = Math.max(DESKTOP_LYRICS_FONT_MIN, Math.min(DESKTOP_LYRICS_FONT_MAX, val));
    void updateDesktop({ font_size: clamped });
  };

  const handleDesktopLockToggle = () => {
    void updateDesktop({ locked: !desktopSettings.locked });
  };

  const handleDesktopOpacity = (val: number) => {
    const clamped = Math.max(DESKTOP_LYRICS_OPACITY_MIN, Math.min(DESKTOP_LYRICS_OPACITY_MAX, val));
    void updateDesktop({ opacity: clamped });
  };

  const handleDesktopReset = async () => {
    await desktopLyricsReset().catch((e) => logWarn('desktopLyricsReset', e));
  };

  const handleVolumeChange = async (val: number) => {
    const clamped = Math.max(0, Math.min(100, val));
    setVolume(clamped);
    await ipcSetVolume(clamped);
  };

  const handleCrossfadeChange = async (val: number) => {
    setCrossfadeMs(val);
    await crossfadeSetDuration(val);
  };

  const handleSkinChange = async (skinId: string) => {
    await applySkin(skinId);
  };

  // --- Lyrics API token ---
  const handleSaveToken = async () => {
    setTokenError('');
    setTokenSuccess('');
    const trimmed = apiToken.trim();
    if (!trimmed) {
      setTokenError('Token 不能为空');
      return;
    }
    setTokenSaving(true);
    try {
      await lyricsSetToken(trimmed);
      setApiToken(trimmed);
      setApiTokenMasked(trimmed.length > 8
        ? `${trimmed.slice(0, 4)}****${trimmed.slice(-4)}`
        : '****');
      setTokenSuccess('Token 已保存并生效');
      setTimeout(() => setTokenSuccess(''), 3000);
    } catch (e: any) {
      setTokenError(typeof e === 'string' ? e : (e?.message ?? '保存失败'));
    } finally {
      setTokenSaving(false);
    }
  };

  const handleClearToken = async () => {
    setTokenError('');
    setTokenSuccess('');
    try {
      await lyricsSetToken('');
      setApiToken('');
      setApiTokenMasked('');
      setTokenSuccess('Token 已清除');
      setTimeout(() => setTokenSuccess(''), 3000);
    } catch (e: any) {
      setTokenError(typeof e === 'string' ? e : (e?.message ?? '清除失败'));
    }
  };

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.panel} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>⚙️ 设置</span>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>

        <div className={styles.tabs}>
          <button
            className={`${styles.tab} ${activeTab === 'audio' ? styles.activeTab : ''}`}
            onClick={() => setActiveTab('audio')}
          >🔊 音频</button>
          <button
            className={`${styles.tab} ${activeTab === 'lyrics' ? styles.activeTab : ''}`}
            onClick={() => setActiveTab('lyrics')}
          >🎤 歌词</button>
          <button
            className={`${styles.tab} ${activeTab === 'skin' ? styles.activeTab : ''}`}
            onClick={() => setActiveTab('skin')}
          >🎨 外观</button>
          <button
            className={`${styles.tab} ${activeTab === 'about' ? styles.activeTab : ''}`}
            onClick={() => setActiveTab('about')}
          >ℹ️ 关于</button>
        </div>

        <div className={styles.content}>
          {activeTab === 'audio' && (
            <div className={styles.section}>
              <div className={styles.row}>
                <label className={styles.label}>主音量</label>
                <div className={styles.sliderRow}>
                  <input
                    type="range"
                    min={0} max={100}
                    value={volume}
                    onChange={(e) => handleVolumeChange(parseInt(e.target.value))}
                    className={styles.slider}
                  />
                  <span className={styles.value}>{volume}%</span>
                </div>
              </div>
              <div className={styles.row}>
                <label className={styles.label}>交叉淡入淡出</label>
                <div className={styles.sliderRow}>
                  <input
                    type="range"
                    min={0} max={10000} step={500}
                    value={crossfadeMs}
                    onChange={(e) => handleCrossfadeChange(parseInt(e.target.value))}
                    className={styles.slider}
                  />
                  <span className={styles.value}>{(crossfadeMs / 1000).toFixed(1)}s</span>
                </div>
              </div>
            </div>
          )}

          {activeTab === 'lyrics' && (
            <div className={styles.section}>
              <div className={styles.row}>
                <label className={styles.label}>API Token</label>
                <span className={styles.muted}>
                  前往 <a href="https://openapi.52vmy.cn" target="_blank" rel="noopener noreferrer" style={{ color: 'var(--accent)' }}>openapi.52vmy.cn</a> 注册获取 Token，用于在线歌词搜索。
                </span>
              </div>

              {apiTokenMasked && (
                <div className={styles.row}>
                  <label className={styles.label}>当前 Token</label>
                  <span className={styles.muted} style={{ fontFamily: 'monospace', userSelect: 'all' }}>
                    {apiTokenMasked}
                  </span>
                </div>
              )}

              <div className={styles.addServerRow}>
                <input
                  className={styles.addServerInput}
                  value={apiToken}
                  onChange={(e) => {
                    setApiToken(e.target.value);
                    setTokenError('');
                    setTokenSuccess('');
                  }}
                  onKeyDown={(e) => e.key === 'Enter' && handleSaveToken()}
                  placeholder="请输入 API Token"
                  disabled={tokenSaving}
                  type="password"
                  autoComplete="off"
                />
                <button
                  className={styles.addServerBtn}
                  onClick={handleSaveToken}
                  disabled={tokenSaving || !apiToken.trim()}
                >{tokenSaving ? '保存中...' : '保存'}</button>
                {apiTokenMasked && (
                  <button
                    className={styles.addServerBtn}
                    onClick={handleClearToken}
                    style={{ background: 'rgba(255,100,100,0.2)' }}
                    disabled={tokenSaving}
                  >清除</button>
                )}
              </div>

              {tokenError && (
                <div className={styles.row}>
                  <span style={{ color: '#f87171', fontSize: 13 }}>{tokenError}</span>
                </div>
              )}
              {tokenSuccess && (
                <div className={styles.row}>
                  <span style={{ color: '#34d399', fontSize: 13 }}>{tokenSuccess}</span>
                </div>
              )}

              <div className={styles.row}>
                <span className={styles.muted}>
                  Token 会加密保存在本地，填写后即可使用在线歌词搜索功能。Token 格式为字母、数字、下划线和连字符的组合。
                </span>
              </div>

              <hr className={styles.divider} />

              {/* ─── 主界面歌词样式 ─── */}
              <div className={styles.row}>
                <label className={styles.label}>主界面歌词样式</label>
                <span className={styles.muted}>
                  字体与对齐方式实时生效，同时应用于歌词列表整体布局与单行显示。
                </span>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>字体</label>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                  <select
                    className={styles.addServerInput}
                    style={{ flex: '0 0 auto', width: 'auto', minWidth: 200 }}
                    value={mainLyricsFontFamily}
                    onChange={(e) => setMainLyricsFontFamily(e.target.value)}
                  >
                    {/* 当前值若不在任何列表中，仍保持可选（避免显示空白） */}
                    {![...systemFonts, ...FONT_FAMILY_OPTIONS].some((o) => o.value === mainLyricsFontFamily) && (
                      <option value={mainLyricsFontFamily}>{mainLyricsFontFamily}</option>
                    )}
                    {systemFonts.length > 0 && (
                      <optgroup label="系统字体">
                        {systemFonts.map((opt) => (
                          <option key={opt.value} value={opt.value}>{opt.label}</option>
                        ))}
                      </optgroup>
                    )}
                    <optgroup label="精选字体">
                      {FONT_FAMILY_OPTIONS.map((opt) => (
                        <option key={opt.value} value={opt.value}>{opt.label}</option>
                      ))}
                    </optgroup>
                  </select>
                  <button
                    className={styles.addServerBtn}
                    onClick={() => void loadSystemFonts()}
                    disabled={fontsLoading}
                    title="重新扫描系统已安装字体（TrueType / OpenType 等）"
                    style={{ padding: '8px 12px', fontSize: 12 }}
                  >{fontsLoading ? '加载中...' : '🔄 刷新'}</button>
                </div>
                {fontsLoadError && (
                  <span className={styles.muted} style={{ color: '#fbbf24' }}>{fontsLoadError}</span>
                )}
              </div>

              <div className={styles.row}>
                <label className={styles.label}>字号</label>
                <div className={styles.sliderRow}>
                  <input
                    type="range"
                    min={10}
                    max={32}
                    step={1}
                    value={mainLyricsFontSize}
                    onChange={(e) => setMainLyricsFontSize(parseInt(e.target.value))}
                    className={styles.slider}
                  />
                  <span className={styles.value}>{mainLyricsFontSize}px</span>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>行高</label>
                <div className={styles.sliderRow}>
                  <input
                    type="range"
                    min={1.2}
                    max={3.0}
                    step={0.1}
                    value={mainLyricsLineHeight}
                    onChange={(e) => setMainLyricsLineHeight(parseFloat(e.target.value))}
                    className={styles.slider}
                  />
                  <span className={styles.value}>{mainLyricsLineHeight.toFixed(1)}</span>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>文字对齐</label>
                <div style={{ display: 'flex', gap: 6 }}>
                  {LYRICS_TEXT_ALIGN_OPTIONS.map((opt) => (
                    <button
                      key={opt.value}
                      className={`${styles.skinCard} ${mainLyricsTextAlign === opt.value ? styles.activeSkin : ''}`}
                      onClick={() => setMainLyricsTextAlign(opt.value)}
                      style={{ padding: '6px 12px', fontSize: 12 }}
                    >{opt.label}</button>
                  ))}
                </div>
              </div>

              <hr className={styles.divider} />

              <div className={styles.row}>
                <label className={styles.label}>桌面歌词</label>
                <span className={styles.muted}>
                  字体、字号、样式与颜色设置实时同步到桌面歌词窗口。
                </span>
              </div>

              {/* 实时预览：反映当前所有字体设置 */}
              <div className={styles.row}>
                <label className={styles.label}>预览</label>
                <div style={{
                  padding: '10px 14px',
                  background: 'rgba(0,0,0,0.3)',
                  borderRadius: 10,
                  border: '1px solid rgba(255,255,255,0.08)',
                  flex: 1,
                  overflow: 'hidden',
                }}>
                  <span style={{
                    fontSize: Math.min(desktopSettings.font_size, 22),
                    fontFamily: desktopSettings.font_family,
                    fontWeight: desktopSettings.bold ? 700 : 400,
                    fontStyle: desktopSettings.italic ? 'italic' : 'normal',
                    color: desktopSettings.font_color,
                    textShadow: `0 0 8px ${desktopSettings.font_color}66`,
                    whiteSpace: 'nowrap',
                  }}>
                    示例歌词文本 Sample Lyrics
                  </span>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>字体类型</label>
                <select
                  className={styles.addServerInput}
                  style={{ flex: '0 0 auto', width: 'auto', minWidth: 160 }}
                  value={desktopSettings.font_family}
                  onChange={(e) => updateDesktop({ font_family: e.target.value })}
                >
                  {FONT_FAMILY_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>{opt.label}</option>
                  ))}
                </select>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>字号</label>
                <div className={styles.sliderRow}>
                  <input
                    type="range"
                    min={DESKTOP_LYRICS_FONT_MIN}
                    max={DESKTOP_LYRICS_FONT_MAX}
                    step={2}
                    value={desktopSettings.font_size}
                    onChange={(e) => handleDesktopFontSize(parseInt(e.target.value))}
                    className={styles.slider}
                  />
                  <span className={styles.value}>{desktopSettings.font_size}px</span>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>样式</label>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button
                    className={`${styles.skinCard} ${!desktopSettings.bold && !desktopSettings.italic ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ bold: false, italic: false })}
                    style={{ padding: '6px 12px', fontSize: 12 }}
                  >常规</button>
                  <button
                    className={`${styles.skinCard} ${desktopSettings.bold && !desktopSettings.italic ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ bold: true, italic: false })}
                    style={{ padding: '6px 12px', fontSize: 12, fontWeight: 700 }}
                  >粗体</button>
                  <button
                    className={`${styles.skinCard} ${!desktopSettings.bold && desktopSettings.italic ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ bold: false, italic: true })}
                    style={{ padding: '6px 12px', fontSize: 12, fontStyle: 'italic' }}
                  >斜体</button>
                  <button
                    className={`${styles.skinCard} ${desktopSettings.bold && desktopSettings.italic ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ bold: true, italic: true })}
                    style={{ padding: '6px 12px', fontSize: 12, fontWeight: 700, fontStyle: 'italic' }}
                  >粗斜体</button>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>颜色</label>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                  <input
                    type="color"
                    value={desktopSettings.font_color}
                    onChange={(e) => updateDesktop({ font_color: e.target.value })}
                    title="自定义颜色"
                    style={{ width: 32, height: 32, padding: 0, border: 'none', background: 'transparent', cursor: 'pointer' }}
                  />
                  <input
                    type="text"
                    className={styles.addServerInput}
                    style={{ flex: '0 0 auto', width: 90, fontFamily: 'monospace' }}
                    value={desktopSettings.font_color}
                    onChange={(e) => {
                      const v = e.target.value;
                      // 允许输入中临时不完整，失焦/Enter 时校验
                      setDesktopSettings((prev) => ({ ...prev, font_color: v }));
                    }}
                    onBlur={(e) => {
                      const v = e.target.value.trim();
                      if (/^#[0-9A-Fa-f]{6}$/.test(v)) {
                        updateDesktop({ font_color: v });
                      } else {
                        // 非法，回滚到服务端值（触发重读）
                        desktopLyricsGet().then(setDesktopSettings).catch(() => {});
                      }
                    }}
                    maxLength={7}
                  />
                  <span style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
                    {FONT_COLOR_PRESETS.map((c) => (
                      <button
                        key={c}
                        onClick={() => updateDesktop({ font_color: c })}
                        title={c}
                        style={{
                          width: 20, height: 20, padding: 0, borderRadius: '50%',
                          border: desktopSettings.font_color.toLowerCase() === c.toLowerCase()
                            ? '2px solid #fff'
                            : '2px solid transparent',
                          background: c, cursor: 'pointer',
                          boxShadow: desktopSettings.font_color.toLowerCase() === c.toLowerCase()
                            ? '0 0 0 1px rgba(255,255,255,0.5)'
                            : 'none',
                          transition: 'border-color 0.15s',
                        }}
                      />
                    ))}
                  </span>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>窗口位置</label>
                <button
                  className={`${styles.skinCard} ${desktopSettings.locked ? styles.activeSkin : ''}`}
                  onClick={handleDesktopLockToggle}
                  style={{ padding: '6px 12px', fontSize: 12 }}
                  title={desktopSettings.locked ? '已锁定，窗口位置不可拖动' : '未锁定，可拖动窗口'}
                >
                  {desktopSettings.locked ? '🔒 已锁定位置' : '🔓 可自由拖动'}
                </button>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>窗口不透明度</label>
                <div className={styles.sliderRow}>
                  <input
                    type="range"
                    min={DESKTOP_LYRICS_OPACITY_MIN}
                    max={DESKTOP_LYRICS_OPACITY_MAX}
                    step={0.05}
                    value={desktopSettings.opacity}
                    onChange={(e) => handleDesktopOpacity(parseFloat(e.target.value))}
                    className={styles.slider}
                  />
                  <span className={styles.value}>{Math.round(desktopSettings.opacity * 100)}%</span>
                </div>
              </div>

              <hr className={styles.divider} />

              <div className={styles.row}>
                <label className={styles.label}>卡拉OK逐字</label>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button
                    className={`${styles.skinCard} ${desktopSettings.karaoke ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ karaoke: true })}
                    style={{ padding: '6px 12px', fontSize: 12 }}
                  >开启</button>
                  <button
                    className={`${styles.skinCard} ${!desktopSettings.karaoke ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ karaoke: false })}
                    style={{ padding: '6px 12px', fontSize: 12 }}
                  >关闭</button>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>显示行数</label>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button
                    className={`${styles.skinCard} ${desktopSettings.line_count === 1 ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ line_count: 1 })}
                    style={{ padding: '6px 12px', fontSize: 12 }}
                  >单行</button>
                  <button
                    className={`${styles.skinCard} ${desktopSettings.line_count === 2 ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ line_count: 2 })}
                    style={{ padding: '6px 12px', fontSize: 12 }}
                  >双行</button>
                </div>
              </div>

              <div className={styles.row}>
                <label className={styles.label}>显示方向</label>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button
                    className={`${styles.skinCard} ${desktopSettings.direction === 'horizontal' ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ direction: 'horizontal' })}
                    style={{ padding: '6px 12px', fontSize: 12 }}
                  >横向</button>
                  <button
                    className={`${styles.skinCard} ${desktopSettings.direction === 'vertical' ? styles.activeSkin : ''}`}
                    onClick={() => updateDesktop({ direction: 'vertical' })}
                    style={{ padding: '6px 12px', fontSize: 12 }}
                  >竖向</button>
                </div>
              </div>

              <div className={styles.row}>
                <button
                  className={styles.addServerBtn}
                  onClick={handleDesktopReset}
                  title="恢复字号/字体/样式/颜色/锁定/卡拉OK/显示模式为默认"
                >↺ 恢复默认</button>
              </div>
            </div>
          )}

          {activeTab === 'skin' && (
            <div className={styles.section}>
              <div className={styles.row}>
                <label className={styles.label}>主题模式</label>
                <div style={{ display: 'flex', gap: 6 }}>
                  {(['dark', 'light', 'system'] as const).map((m) => {
                    const labels: Record<string, string> = { dark: '🌙 深色', light: '☀️ 浅色', system: '💻 跟随系统' };
                    return (
                      <button
                        key={m}
                        className={`${styles.skinCard} ${themeMode === m ? styles.activeSkin : ''}`}
                        onClick={async () => {
                          setThemeMode(m);
                          await themeSetMode(m);
                          const resolved = m === 'system'
                            ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
                            : m;
                          document.documentElement.setAttribute('data-theme', resolved);
                          // Sync to desktop lyrics window
                          void emitTo('lyrics-desktop', 'theme-changed', { mode: m })
                            .then(() => console.log('[TTPlayer] theme-changed emitted, mode =', m))
                            .catch((e: unknown) => console.error('[TTPlayer] theme-changed emit failed:', e));
                        }}
                        style={{ padding: '6px 10px', fontSize: 12 }}
                      >
                        {labels[m]}
                      </button>
                    );
                  })}
                </div>
              </div>
              <hr className={styles.divider} />
              <label className={styles.label}>皮肤</label>
              <div className={styles.skinGrid}>
                {availableSkins.map((skin) => (
                  <button
                    key={skin.id}
                    className={`${styles.skinCard} ${skin.id === currentSkinId ? styles.activeSkin : ''}`}
                    onClick={() => handleSkinChange(skin.id)}
                  >
                    <span className={styles.skinName}>{skin.name}</span>
                    <span className={styles.skinDesc}>{skin.description}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {activeTab === 'about' && (
            <div className={styles.section}>
              <div className={styles.aboutContent}>
                <h3>🎵 TTPlayer-Next</h3>
                <p>版本 0.1.0</p>
                <p>基于 Tauri 2.0 + React 19 + Rust</p>
                <p>致敬千千静听 TTPlayer 5.7.9</p>
                <hr className={styles.divider} />
                <p className={styles.muted}>支持格式：FLAC / MP3 / AAC / APE / AC-3 / OGG / Opus / WAV / MOD / XM / S3M / IT</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
