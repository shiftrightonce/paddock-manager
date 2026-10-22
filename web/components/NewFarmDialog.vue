<script setup>
import { ref } from 'vue';
import { api } from '../api.js';
import { guard, loadFarms, store, toast } from '../composables/store.js';
import ModalDialog from './ModalDialog.vue';

const props = defineProps({
  farm: { type: Object, default: null },
});
const emit = defineEmits(['cancel', 'saved']);

const name = ref(props.farm?.name || '');
const location = ref(props.farm?.location || '');
const weatherStation = ref(props.farm?.weather_station || '');
const isEdit = ref(Boolean(props.farm?.id));
const busy = ref(false);
const error = ref('');

async function save() {
  if (!name.value.trim()) {
    error.value = 'A farm name is required.';
    return;
  }
  busy.value = true;
  error.value = '';
  await guard(async () => {
    const payload = {
      name: name.value.trim(),
      location: location.value.trim(),
      weather_station: weatherStation.value.trim(),
    };
    if (isEdit.value) {
      await api.put(`/api/farms/${props.farm.id}`, payload);
      toast('Farm updated', 'info');
    } else {
      const created = await api.post('/api/farms', payload);
      store.farms.push(created);
      store.currentFarmId = created.id;
      toast('Farm added', 'info');
    }
    await loadFarms();
    emit('saved');
  }, { errorMessage: 'Could not save the farm' });
  busy.value = false;
}

async function remove() {
  if (!window.confirm(`Delete ${props.farm.name} and all of its fields? This cannot be undone.`)) return;
  busy.value = true;
  await guard(async () => {
    await api.del(`/api/farms/${props.farm.id}`);
    toast('Farm deleted', 'info');
    await loadFarms();
    emit('saved');
  }, { errorMessage: 'Could not delete the farm' });
  busy.value = false;
}
</script>

<template>
  <ModalDialog
    :title="isEdit ? 'Edit farm' : 'Add a farm'"
    :confirm-label="isEdit ? 'Save changes' : 'Add farm'"
    :busy="busy"
    @cancel="emit('cancel')"
    @confirm="save"
  >
    <div class="stack">
      <div v-if="error" class="alert alert--red">
        <span class="alert__icon">!</span>
        <div>{{ error }}</div>
      </div>

      <div class="field">
        <label>Farm name</label>
        <input v-model="name" class="input" placeholder="Riverbend" />
      </div>

      <div class="field">
        <label>Location</label>
        <input v-model="location" class="input" placeholder="Yenda, NSW" />
        <div class="field__hint">Used for BOM weather station suggestions.</div>
      </div>

      <div class="field">
        <label>BOM weather station number</label>
        <input v-model="weatherStation" class="input" inputmode="numeric" placeholder="049017" />
        <div class="field__hint">Optional. Shown as a reference only - rainfall is still entered by hand.</div>
      </div>

      <div v-if="isEdit" class="alert alert--amber">
        <span class="alert__icon">!</span>
        <div class="grow">
          <button class="btn btn--danger btn--sm" @click="remove">Delete this farm</button>
          <div class="tiny" style="margin-top: 6px">Removes the farm, its fields, entries and tasks.</div>
        </div>
      </div>
    </div>
  </ModalDialog>
</template>