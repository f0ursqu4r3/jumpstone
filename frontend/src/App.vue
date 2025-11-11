<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute } from 'vue-router'

import AuthLayout from '~/layouts/AuthLayout.vue'
import DefaultLayout from '~/layouts/DefaultLayout.vue'
import MessagesLayout from '~/layouts/MessagesLayout.vue'

const route = useRoute()

const layoutComponent = computed(() => {
  const layout = route.meta.layout
  if (layout === 'auth') {
    return AuthLayout
  }
  if (layout === 'messages') {
    return MessagesLayout
  }
  return DefaultLayout
})
</script>

<template>
  <UApp>
    <component :is="layoutComponent">
      <RouterView />
    </component>
  </UApp>
</template>
