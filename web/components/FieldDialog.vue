<script setup>
import { ref } from 'vue';
import { api } from '../api.js';
import { guard, toast } from '../composables/store.js';
import ModalDialog from './ModalDialog.vue';

const props = defineProps({
  field: { type: Object, default: null },
  farmId: { type: Number, required: true },
  soils: { type: Array, default: () => [] },
});
const emit = defineEmits(['cancel', 'saved']);

const isEdit = ref(Boolean(props.field?.id));
const name = ref(props.field?.name || '');
const area = ref(props.field?.area_ha ?? null);
const soilType = ref(props.field?.soil_type || '');
const notes = ref(props.field?.notes || '');
const busy = ref(false);
const error = ref('');

async function save() {
  if (!name.value.trim()) {
    error.value = 'A field name is required.';
    return;
  }
  busy.value = true;
  error.value = '';
  await guard(async () => {
    const payload = {
      farm_id: props.farmId,
      name: name.value.trim(),
      area_ha: area.value === '' ? null : Number(area.value),
      soil_type: soilType.value,
      notes: notes.value,
    };
    if (isEdit.value) {
      await api.put(`/api/fields/${props.field.id}`, payload);
      toast('Field updated', 'info');
    } else {
      await api.post('/api/fields', payload);
      toast('Field added', 'info');
    }
    emit('saved');
  }, { errorMessage: 'Could not save the field' });
  busy.value = false;
}
</script>

<template>
  <ModalDialog
    :title="isEdit ? 'Edit field' : 'Add a field'"
    :confirm-label="isEdit ? 'Save changes' : 'Add field'"
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
        <label>Field name</label>
        <input v-model="name" class="input" placeholder="North paddock" />
      </div>

      <div class="grid-2">
        <div class="field">
          <label>Area (ha)</label>
          <input v-model.number="area" class="input" type="number" step="0.1" inputmode="decimal" placeholder="32.5" />
        </div>
        <div class="field">
          <label>Soil type</label>
          <input
            v-model="soilType"
            class="input"
            list="soil-options"
            placeholder="Clay loam"
          />
          <datalist id="soil-options">
            <option v-for="s in soils" :key="s" :value="s" />
          </datalist>
        </div>
      </div>

      <div class="field">
        <label>Notes</label>
        <textarea v-model="notes" class="textarea" placeholder="Fencing, drainage, anything to remember" />
      </div>
    </div>
  </ModalDialog>
</template>