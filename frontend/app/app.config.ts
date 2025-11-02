export default defineAppConfig({
  ui: {
    primary: 'brand',
    gray: 'slate',
    icons: {
      dynamic: 'true',
    },
    button: {
      defaultVariants: {
        color: 'primary',
        variant: 'soft',
        size: 'md',
      },
    },
    badge: {
      defaultVariants: {
        color: 'primary',
        variant: 'soft',
      },
    },
    variables: {
      'font-family-base': 'var(--font-family-base)',
      'font-size-base': '12pt',
      'border-radius': 'var(--radius-lg)',
      'color-primary-50': 'var(--color-brand-50)',
      'color-primary-100': 'var(--color-brand-100)',
      'color-primary-200': 'var(--color-brand-200)',
      'color-primary-300': 'var(--color-brand-300)',
      'color-primary-400': 'var(--color-brand-400)',
      'color-primary-500': 'var(--color-brand-500)',
      'color-primary-600': 'var(--color-brand-600)',
      'color-primary-700': 'var(--color-brand-700)',
      'color-primary-800': 'var(--color-brand-800)',
      'color-primary-900': 'var(--color-brand-900)',
    },
  },
});
