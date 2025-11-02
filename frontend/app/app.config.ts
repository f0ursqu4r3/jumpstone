export default defineAppConfig({
  ui: {
    primary: 'sky',
    gray: 'slate',
    icons: {
      dynamic: 'true',
    },
    button: {
      defaultVariants: {
        color: 'neutral',
      },
    },
    variables: {
      'font-family-base':
        'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
      'font-size-base': '12pt',
    },
  },
});
