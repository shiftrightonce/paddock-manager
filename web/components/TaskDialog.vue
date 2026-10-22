<script setup>
import { computed, ref, watch } from 'vue';
import { api } from '../api.js';
import { toast } from '../composables/store.js';
import ModalDialog from './ModalDialog.vue';

/**
 * Schedule a task: field-linked (writes an Inspection entry on completion) or
 * whole-farm, with optional weekly / fortnightly / monthly repeat.
 */
const props = defineProps({
  farms: { type: Array, default: () => [] },
  fields: { type: Array, default: () => [] },
  presetFieldId: { type: Number, default: null },
  presetFarmId: { type: Number, default: null },
});
const emit = defineEmits(['cancel', 'saved']);

const farmId = ref(props.presetFarmId ? Number(props.presetFarmId) : Number(props.farms[0]?.id || 0));
const fieldId = ref(props.presetFieldId ? Number(props.presetFieldId) : null);
const taskType = ref('insect_scouting');
const title = ref('');
const notes = ref('');
const dueDate = ref(new Date().toISOString().slice(0, 10));
const repeat = ref('none');
const busy = ref(false);
const error = ref('');

const fieldsForFarm = computed(() => props.fields.filter((f) => f.farm_id === farmId.value));

watch(farmId, () => {
  if (fieldId.value && !fieldsForFarm.value.some((f) => f.id === fieldId.value)) fieldId.value = null;
});

watch(taskType, (t) => {
  if (!title.value.trim()) {
    title.value =
      t === 'insect_scouting' ? 'Scout for insects' : t === 'crop_inspection' ? 'Crop inspection' : '';
  }
});

async function save() {
  if (!title.value.trim()) {
    error.value = 'Give the task a name.';
    return;
  }
  if (!farmId.value) {
    error.value = 'Pick a farm.';
    return;
  }
  busy.value = true;
  error.value = '';
  try {
    await api.post('/api/tasks', {
      farmId: farmId.value,
      fieldId: fieldId.value || null,
      taskType: taskType.value,
      title: title.value.trim(),
      notes: notes.value,
      due_date: dueDate.value,
      repeat_interval: repeat.value,
    });
    toast('Task scheduled', 'info');
    emit('saved');
  } catch (err) {
    error.value = err.message;
  } finally {
    busy.value = false;
  }
}

const TYPE_LABEL = {
  insect_scouting: 'Insect scouting',
  crop_inspection: 'Crop inspection',
  custom: 'Custom',
};
</script>

<template>
  <ModalDialog title="Schedule a task" confirm-label="Schedule" :busy="busy" @cancel="emit('cancel')" @confirm="save">
    <div class="stack">
      <div v-if="error" class="alert alert--red">
        <span class="alert__icon">!</span>
        <div>{{ error }}</div>
      </div>

      <div class="field">
        <label>Task type</label>
        <select v-model="taskType" class="select">
          <option v-for="(label, key) in TYPE_LABEL" :key="key" :value="key">{{ label }}</option>
        </select>
      </div>

      <div class="field">
        <label>Task name</label>
        <input v-model="title" class="input" placeholder="Scout for insects" />
      </div>

      <div class="grid-2">
        <div class="field">
          <label>Farm</label>
          <select v-model="farmId" class="select">
            <option v-for="f in farms" :key="f.id" :value="f.id">{{ f.name }}</option>
          </select>
        </div>
        <div class="field">
          <label>Field</label>
          <select v-model="fieldId" class="select">
            <option :value="null">Whole farm</option>
            <option v-for="f in fieldsForFarm" :key="f.id" :value="f.id">{{ f.name }}</option>
          </select>
        </div>
      </div>

      <div class="grid-2">
        <div class="field">
          <label>Due date</label>
          <input v-model="dueDate" class="input" type="date" />
        </div>
        <div class="field">
          <label>Repeat</label>
          <select v-model="repeat" class="select">
            <option value="none">Does not repeat</option>
            <option value="weekly">Weekly</option>
            <option value="fortnightly">Fortnightly</option>
            <option value="monthly">Monthly</option>
          </select>
        </div>
      </div>

      <div class="field">
        <label>Notes</label>
        <textarea v-model="notes" class="textarea" placeholder="What to look for" />
      </div>

      <div class="alert alert--info">
        <span class="alert__icon">i</span>
        <div>
          Completing a field-linked task writes an Inspection entry to that field's timeline.
          Repeating tasks log a fresh entry each time and reschedule. Whole-farm tasks complete
          without a field entry.
        </div>
      </div>
    </div>
  </ModalDialog>
</template>