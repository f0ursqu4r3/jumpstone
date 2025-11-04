<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

import Button from '@/components/ui/Button.vue'

interface OnboardingSlide {
  id: string
  title: string
  description: string
  ctaLabel: string
  href?: string
  to?: RouteLocationRaw
  eyebrow?: string
  icon?: string
}

const props = withDefaults(
  defineProps<{
    slides: OnboardingSlide[]
    intervalMs?: number
    autoPlay?: boolean
  }>(),
  {
    intervalMs: 9000,
    autoPlay: true,
  },
)

const activeIndex = ref(0)
const isHovering = ref(false)
const timerId = ref<number | null>(null)

const totalSlides = computed(() => props.slides.length)
const autoPlayEnabled = computed(
  () => props.autoPlay && totalSlides.value > 1,
)

const currentSlide = computed(() => props.slides[activeIndex.value] ?? null)
const hasCta = computed(
  () => Boolean(currentSlide.value?.href || currentSlide.value?.to),
)
const buttonAttrs = computed<Record<string, unknown>>(() => {
  const slide = currentSlide.value
  if (!slide) {
    return {}
  }

  if (slide.href) {
    return {
      href: slide.href,
      target: '_blank',
      rel: 'noopener',
    }
  }

  if (slide.to) {
    return {
      to: slide.to,
    }
  }

  return {}
})

const normalizeIndex = (index: number) => {
  if (!totalSlides.value) {
    return 0
  }
  const value = index % totalSlides.value
  return value < 0 ? value + totalSlides.value : value
}

const goTo = (index: number) => {
  activeIndex.value = normalizeIndex(index)
}

const next = () => {
  goTo(activeIndex.value + 1)
}

const previous = () => {
  goTo(activeIndex.value - 1)
}

const stopTimer = () => {
  if (timerId.value !== null && typeof window !== 'undefined') {
    window.clearInterval(timerId.value)
    timerId.value = null
  }
}

const startTimer = () => {
  if (!autoPlayEnabled.value || isHovering.value || typeof window === 'undefined') {
    return
  }

  stopTimer()
  timerId.value = window.setInterval(() => {
    next()
  }, props.intervalMs)
}

const handleSelect = (index: number) => {
  goTo(index)
  if (autoPlayEnabled.value) {
    stopTimer()
    startTimer()
  }
}

const handleMouseEnter = () => {
  isHovering.value = true
}

const handleMouseLeave = () => {
  isHovering.value = false
}

const previousLabel = computed(() => {
  if (!totalSlides.value) {
    return 'Previous slide'
  }
  const index = normalizeIndex(activeIndex.value - 1)
  return props.slides[index]?.title ?? 'Previous slide'
})

const nextLabel = computed(() => {
  if (!totalSlides.value) {
    return 'Next slide'
  }
  const index = normalizeIndex(activeIndex.value + 1)
  return props.slides[index]?.title ?? 'Next slide'
})

onMounted(() => {
  startTimer()
})

onUnmounted(() => {
  stopTimer()
})

watch(autoPlayEnabled, (enabled) => {
  if (!enabled) {
    stopTimer()
  } else {
    startTimer()
  }
})

watch(isHovering, (hovering) => {
  if (hovering) {
    stopTimer()
  } else {
    startTimer()
  }
})

watch(
  () => props.intervalMs,
  () => {
    stopTimer()
    startTimer()
  },
)

watch(
  () => props.slides,
  (slides) => {
    if (slides.length === 0) {
      stopTimer()
      activeIndex.value = 0
      return
    }
    goTo(0)
    startTimer()
  },
  { deep: true },
)
</script>

<template>
  <section
    class="flex h-full flex-col justify-between overflow-hidden rounded-2xl border border-white/10 bg-slate-900/60 p-6 shadow-[0_35px_60px_-15px_rgba(2,6,23,0.5)] backdrop-blur"
    role="region"
    aria-label="Onboarding carousel"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
  >
    <header v-if="currentSlide" class="space-y-3">
      <div
        v-if="currentSlide.eyebrow || currentSlide.icon"
        class="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.3em] text-sky-400"
      >
        <UIcon
          v-if="currentSlide.icon"
          :name="currentSlide.icon"
          class="h-4 w-4 text-sky-400"
        />
        <span>{{ currentSlide.eyebrow }}</span>
      </div>
      <h2 class="text-2xl font-semibold leading-tight text-white lg:text-3xl">
        {{ currentSlide.title }}
      </h2>
      <p class="text-sm leading-relaxed text-slate-300">
        {{ currentSlide.description }}
      </p>
    </header>

    <footer class="mt-8 flex flex-col gap-6">
      <Button
        v-if="currentSlide && hasCta"
        v-bind="buttonAttrs"
        variant="soft"
        color="info"
        class="justify-center"
      >
        {{ currentSlide.ctaLabel }}
      </Button>

      <div class="flex flex-col gap-4">
        <div
          class="flex items-center justify-between text-[10px] font-semibold uppercase tracking-[0.35em] text-slate-500"
          v-if="totalSlides > 0"
        >
          <span>Step {{ activeIndex + 1 }} / {{ totalSlides }}</span>
          <div class="flex items-center gap-3">
            <button
              type="button"
              class="rounded-full border border-white/10 p-2 text-slate-300 transition hover:border-sky-400 hover:text-white"
              @click="previous"
              :aria-label="`Previous: ${previousLabel}`"
            >
              <UIcon name="i-heroicons-chevron-left" class="h-4 w-4" />
            </button>
            <button
              type="button"
              class="rounded-full border border-white/10 p-2 text-slate-300 transition hover:border-sky-400 hover:text-white"
              @click="next"
              :aria-label="`Next: ${nextLabel}`"
            >
              <UIcon name="i-heroicons-chevron-right" class="h-4 w-4" />
            </button>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <button
            v-for="(slide, index) in slides"
            :key="slide.id"
            type="button"
            class="h-1 flex-1 rounded-full transition"
            :class="index === activeIndex ? 'bg-sky-400' : 'bg-white/10 hover:bg-white/20'"
            :aria-current="index === activeIndex"
            :aria-label="`Go to slide ${index + 1}: ${slide.title}`"
            @click="handleSelect(index)"
          />
        </div>
      </div>
    </footer>
  </section>
</template>
