<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute } from 'vue-router'

import AuthLayout from '~/layouts/AuthLayout.vue'
import DefaultLayout from '~/layouts/DefaultLayout.vue'

const route = useRoute()

const layoutComponent = computed(() => {
  const layout = route.meta.layout
  return layout === 'auth' ? AuthLayout : DefaultLayout
})
</script>

<template>
  <UApp>
    <component :is="layoutComponent">
      <RouterView />
    </component>
  </UApp>
</template>
