<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';

const props = defineProps({
  title: { type: String, default: '' },
  confirmLabel: { type: String, default: 'Confirm' },
  cancelLabel: { type: String, default: 'Cancel' },
  danger: { type: Boolean, default: false },
  busy: { type: Boolean, default: false },
  hideFooter: { type: Boolean, default: false },
  wide: { type: Boolean, default: false },
});
const emit = defineEmits(['confirm', 'cancel']);

const bodyRef = ref(null);

function onKey(event) {
  if (event.key === 'Escape') emit('cancel');
}

onMounted(() => {
  document.addEventListener('keydown', onKey);
  document.body.style.overflow = 'hidden';
});

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onKey);
  document.body.style.overflow = '';
});
</script>

<template>
  <div class="modal-backdrop" @click.self="emit('cancel')">
    <div
      class="modal"
      :style="props.wide ? 'max-width: 760px' : null"
      role="dialog"
      aria-modal="true"
      :aria-label="title || 'Dialog'"
    >
      <div class="modal__head">
        <div class="modal__title">{{ title }}</div>
        <button class="btn btn--ghost btn--sm" aria-label="Close" @click="emit('cancel')">✕</button>
      </div>
      <div ref="bodyRef" class="modal__body">
        <slot />
      </div>
      <div v-if="!hideFooter" class="modal__foot">
        <button class="btn" :disabled="busy" @click="emit('cancel')">{{ cancelLabel }}</button>
        <button
          class="btn"
          :class="props.danger ? 'btn--danger' : 'btn--primary'"
          :disabled="busy"
          @click="emit('confirm')"
        >
          {{ busy ? 'Working...' : confirmLabel }}
        </button>
      </div>
    </div>
  </div>
</template>