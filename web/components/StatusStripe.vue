<script setup>
/** Coloured status stripe + short reasons, per the spec's roll-up rules. */
defineProps({
  status: { type: String, default: 'green' },
  reasons: { type: Array, default: () => [] },
  size: { type: String, default: 'md' },
});
</script>

<template>
  <div class="stack stack--tight" :style="{ gap: size === 'sm' ? '3px' : '6px' }">
    <span class="chip" :class="`chip--${status}`">
      <span
        :style="{
          width: '7px',
          height: '7px',
          borderRadius: '50%',
          background: 'currentColor',
          display: 'inline-block',
        }"
      />
      {{ status === 'red' ? 'Resistance' : status === 'amber' ? 'Residual' : 'Clear' }}
    </span>
    <div
      v-for="(reason, i) in reasons"
      :key="i"
      class="tiny"
      :class="reason.level === 'red' ? 'field-row__flags' : 'muted'"
      :style="reason.level === 'red' ? { color: 'var(--red)' } : null"
    >
      {{ reason.text }}
    </div>
  </div>
</template>