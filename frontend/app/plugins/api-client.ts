export default defineNuxtPlugin(() => {
  const api = useApiClient()

  return {
    provide: {
      api,
    },
  }
})
