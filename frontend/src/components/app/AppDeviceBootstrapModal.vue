<script setup lang="ts">
import { computed } from 'vue'

import { recordBreadcrumb } from '@/utils/telemetry'

const props = withDefaults(
  defineProps<{
    deviceId?: string | null
    identifier?: string | null
    serverName?: string | null
  }>(),
  {
    deviceId: '',
    identifier: '',
    serverName: 'local server',
  },
)

const open = defineModel<boolean>('open', { default: false })

const sanitizedDeviceId = computed(() => props.deviceId?.trim() || '')
const sanitizedServerName = computed(() => props.serverName?.trim() || 'local-server')
const bootstrapCommand = computed(
  () =>
    `bun run mls:register --device ${sanitizedDeviceId.value || 'new-device'} --server ${sanitizedServerName.value}`,
)

const copyToClipboard = async (value: string, label: string) => {
  if (!value) {
    return
  }

  try {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value)
    } else {
      throw new Error('Clipboard API unavailable')
    }
  } catch (err) {
    if (typeof document === 'undefined') {
      console.warn(`Failed to copy ${label}`, err)
      return
    }

    try {
      const textarea = document.createElement('textarea')
      textarea.value = value
      textarea.setAttribute('readonly', '')
      textarea.style.position = 'absolute'
      textarea.style.left = '-9999px'
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
    } catch (fallbackErr) {
      console.warn(`Failed to copy ${label}`, fallbackErr)
      return
    }
  }

  recordBreadcrumb({
    message: 'Copied device bootstrap snippet',
    category: 'mls.bootstrap',
    level: 'info',
    data: { label },
  })
}

const close = () => {
  open.value = false
}
</script>

<template>
  <UModal
    v-model:open="open"
    size="lg"
    title="Register a new device"
    description="Placeholder guidance until MLS enrolment endpoints land."
  >
    <template #content>
      <div class="space-y-6 p-6">
        <div class="space-y-2">
          <h2 class="text-xl font-semibold text-white">Register a new device</h2>
          <p class="text-sm text-slate-400">
            Follow these steps to prepare MLS key material and avoid repeated handshake prompts.
          </p>
          <UAlert
            color="neutral"
            variant="soft"
            icon="i-heroicons-light-bulb"
            title="CLI integration coming soon"
            description="For now we surface the command stub so you can script enrolment locally."
          />
        </div>

        <ol class="space-y-4">
          <li class="rounded-2xl border border-white/10 bg-slate-950/40 p-4">
            <p class="text-sm font-semibold text-white">1. Confirm device metadata</p>
            <p class="mt-1 text-xs text-slate-400">
              Device IDs are bound to refresh tokens. Use a stable identifier before minting MLS
              key packages.
            </p>
            <div
              class="mt-3 flex items-center justify-between rounded-xl border border-white/5 bg-slate-900/60 px-3 py-2"
            >
              <code class="font-mono text-xs text-slate-100">
                {{ sanitizedDeviceId || 'device-id-not-set' }}
              </code>
              <UButton
                size="xs"
                variant="ghost"
                color="neutral"
                @click="copyToClipboard(sanitizedDeviceId || '', 'device-id')"
              >
                Copy
              </UButton>
            </div>
          </li>

          <li class="rounded-2xl border border-white/10 bg-slate-950/40 p-4">
            <p class="text-sm font-semibold text-white">2. Run the bootstrap command</p>
            <p class="mt-1 text-xs text-slate-400">
              Replace the command once the official enrolment endpoint ships; for now it proxies
              local tooling.
            </p>
            <pre
              class="mt-3 overflow-x-auto rounded-xl border border-white/5 bg-slate-900/60 p-3 text-[11px] text-slate-100"
            >
{{ bootstrapCommand }}
            </pre>
            <div class="mt-2 flex items-center gap-2">
              <UButton
                size="xs"
                variant="ghost"
                color="neutral"
                @click="copyToClipboard(bootstrapCommand, 'bootstrap-command')"
              >
                Copy command
              </UButton>
              <span class="text-[11px] text-slate-500">
                Target server: {{ sanitizedServerName }}
              </span>
            </div>
          </li>

          <li class="rounded-2xl border border-white/10 bg-slate-950/40 p-4">
            <p class="text-sm font-semibold text-white">3. Verify handshake vectors</p>
            <p class="mt-1 text-xs text-slate-400">
              After the device registers, refresh the handshake vectors on the dashboard to store
              the verification timestamp locally.
            </p>
            <p class="mt-2 text-xs text-slate-500">
              Identifier: <span class="font-semibold text-slate-200">{{ identifier }}</span>
            </p>
          </li>
        </ol>

        <div class="flex justify-end gap-2">
          <UButton color="neutral" variant="ghost" @click="close"> Close </UButton>
          <UButton color="info" @click="copyToClipboard(bootstrapCommand, 'bootstrap-command')">
            Copy command
          </UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>
