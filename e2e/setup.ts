import '@testing-library/jest-dom';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

// Mock WebviewWindow
vi.mock('@tauri-apps/api/window', () => ({
  WebviewWindow: vi.fn().mockImplementation(() => ({
    show: vi.fn(),
    close: vi.fn(),
    emit: vi.fn(),
  })),
}));
