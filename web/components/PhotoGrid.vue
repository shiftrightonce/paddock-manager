<script setup>
import { ref } from 'vue';
import { api } from '../api.js';
import { guard, toast } from '../composables/store.js';

/**
 * Photo thumbnail grid with an add tile. Uploads go straight to the entry via
 * multipart, so images stay tied to the timeline entry.
 */
const props = defineProps({
  entryId: { type: [Number, String], required: true },
  photos: { type: Array, default: () => [] },
  readonly: { type: Boolean, default: false },
});
const emit = defineEmits(['changed']);

const fileInput = ref(null);
const busy = ref(false);

async function onPick(event) {
  const files = Array.from(event.target.files || []);
  event.target.value = '';
  if (!files.length) return;
  busy.value = true;
  await guard(async () => {
    const form = new FormData();
    for (const file of files) form.append('photos', file);
    await api.post(`/api/entries/${props.entryId}/photos`, form);
    toast(`Added ${files.length} photo${files.length === 1 ? '' : 's'}`, 'info', 2200);
    emit('changed');
  }, { errorMessage: 'Photo upload failed' });
  busy.value = false;
}

async function remove(photo) {
  await guard(async () => {
    await api.del(`/api/photos/${photo.id}`);
    emit('changed');
  }, { errorMessage: 'Could not remove that photo' });
}

const urlFor = (photo) => `/uploads/${photo.filename}`;
</script>

<template>
  <div class="photos">
    <div v-for="photo in photos" :key="photo.id" class="photo">
      <a :href="urlFor(photo)" target="_blank" rel="noopener">
        <img :src="urlFor(photo)" :alt="photo.original_name || 'Field photo'" loading="lazy" />
      </a>
      <button
        v-if="!readonly"
        class="photo__remove"
        type="button"
        :aria-label="`Remove photo ${photo.original_name || ''}`"
        @click="remove(photo)"
      >
        ✕
      </button>
    </div>

    <button
      v-if="!readonly"
      class="photo-add"
      type="button"
      :disabled="busy"
      :aria-label="busy ? 'Uploading photos' : 'Add photos'"
      @click="fileInput.click()"
    >
      {{ busy ? '...' : 'Add photo' }}
    </button>

    <input ref="fileInput" class="sr-only" type="file" accept="image/*" multiple capture="environment" @change="onPick" />
  </div>
</template>