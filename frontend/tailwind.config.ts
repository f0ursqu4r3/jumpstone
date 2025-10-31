import type { Config } from 'tailwindcss';
import ui from '@nuxthq/ui/tailwind.config';

export default {
  content: [
    './app/**/*.{vue,js,ts}',
    './components/**/*.{vue,js,ts}',
    './layouts/**/*.{vue,js,ts}',
    './pages/**/*.{vue,js,ts}',
    './plugins/**/*.{js,ts}',
    './nuxt.config.{js,ts}',
    './node_modules/@nuxthq/ui/dist/runtime/**/*.{js,ts,vue}',
  ],
  theme: {
    extend: {
      colors: {
        background: {
          DEFAULT: '#050816',
          elevated: '#0b1120',
          overlay: '#16213d',
        },
        surface: {
          muted: '#1f2937',
          subtle: '#111827',
        },
        brand: {
          primary: '#6366f1',
          secondary: '#7c3aed',
          accent: '#38bdf8',
        },
        intent: {
          success: '#22c55e',
          warning: '#f59e0b',
          danger: '#ef4444',
          info: '#0ea5e9',
        },
      },
      fontFamily: {
        sans: [
          '"Inter"',
          'system-ui',
          '-apple-system',
          'BlinkMacSystemFont',
          '"Segoe UI"',
          'sans-serif',
        ],
        mono: [
          '"JetBrains Mono"',
          '"Fira Code"',
          'SFMono-Regular',
          'ui-monospace',
          'monospace',
        ],
      },
      spacing: {
        18: '4.5rem',
        72: '18rem',
        84: '21rem',
        96: '24rem',
      },
      borderRadius: {
        xl: '1rem',
        '2xl': '1.5rem',
      },
      boxShadow: {
        focus: '0 0 0 4px rgba(99, 102, 241, 0.35)',
        'elevated-sm': '0 12px 24px rgba(2, 6, 23, 0.25)',
      },
    },
  },
  plugins: [],
  presets: [ui],
} satisfies Config;
