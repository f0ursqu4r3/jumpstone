import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import axe from 'axe-core'

import AppMessageTimeline from '@/components/app/AppMessageTimeline.vue'

const sampleEvents = [
  {
    sequence: 1,
    channel_id: 'channel-1',
    event: {
      schema_version: 1,
      event_id: 'event-1',
      event_type: 'message',
      room_id: 'channel-1',
      sender: 'Casey Example',
      origin_server: 'localhost',
      origin_ts: Date.now(),
      content: { content: 'Accessibility test message' },
      prev_events: [],
      auth_events: [],
      signatures: {},
    },
    optimistic: false,
    localId: undefined,
    pendingSequence: null,
    createdAt: Date.now(),
    status: undefined,
    statusMessage: null,
    ackedAt: null,
  },
]

const runAxeCheck = async (element: HTMLElement) => {
  const results = await axe.run(element, {
    runOnly: {
      type: 'tag',
      values: ['wcag2a', 'wcag2aa'],
    },
  })

  const severeViolations = results.violations.filter((violation) =>
    ['serious', 'critical'].includes(violation.impact ?? ''),
  )

  expect(severeViolations).toHaveLength(0)
}

describe('Accessibility regressions', () => {
  it('AppMessageTimeline renders without serious axe violations', async () => {
    const wrapper = mount(AppMessageTimeline, {
      props: {
        channelName: 'general',
        events: sampleEvents,
      },
      global: {
        stubs: {
          UButton: { template: '<button><slot /></button>' },
          UIcon: { template: '<span><slot /></span>' },
          UBadge: { template: '<span><slot /></span>' },
          USkeleton: { template: '<span></span>' },
          UAlert: { template: '<div><slot /></div>' },
        },
      },
    })

    await runAxeCheck(wrapper.element as HTMLElement)
  })
})
