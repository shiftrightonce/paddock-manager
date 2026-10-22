<script setup>
import { computed, onMounted, ref } from 'vue';
import { api } from '../api.js';
import { guard, loadFarms, refreshBadge, store, toast } from '../composables/store.js';
import { fmt } from '../composables/format.js';
import TaskDialog from '../components/TaskDialog.vue';

const tasks = ref([]);
const fields = ref([]);
const loading = ref(true);
const showSchedule = ref(false);
const showCompleted = ref(false);
const farmFilter = ref('all');

const today = new Date().toISOString().slice(0, 10);

const farmsById = computed(() => new Map(store.farms.map((f) => [f.id, f])));

const filtered = computed(() =>
  farmFilter.value === 'all' ? tasks.value : tasks.value.filter((t) => Number(t.farm_id) === Number(farmFilter.value)),
);

const buckets = computed(() => {
  const overdue = [];
  const todayList = [];
  const week = [];
  const later = [];
  const done = [];
  const horizon = new Date();
  horizon.setDate(horizon.getDate() + 7);
  const horizonIso = horizon.toISOString().slice(0, 10);

  for (const t of filtered.value) {
    if (t.completed_at) done.push(t);
    else if (t.due_date < today) overdue.push(t);
    else if (t.due_date === today) todayList.push(t);
    else if (t.due_date <= horizonIso) week.push(t);
    else later.push(t);
  }
  return { overdue, today: todayList, week, later, done };
});

async function load() {
  loading.value = true;
  await guard(async () => {
    if (!store.farms.length) await loadFarms();
    const [taskList, fieldList] = await Promise.all([
      api.get(`/api/tasks?includeCompleted=${showCompleted.value}`),
      Promise.all(store.farms.map((f) => api.get(`/api/fields?farmId=${f.id}`))),
    ]);
    tasks.value = taskList;
    fields.value = fieldList.flat();
  });
  loading.value = false;
}

async function complete(task) {
  await guard(async () => {
    const result = await api.post(`/api/tasks/${task.id}/complete`);
    if (result.entry) {
      toast(`Logged an inspection entry on ${task.field_name}`, 'info');
    } else {
      toast('Task marked done', 'info');
    }
    if (result.rescheduledTo) toast(`Repeats on ${result.rescheduledTo}`, 'info');
    await load();
    await refreshBadge();
  });
}

async function postpone(task) {
  const d = new Date();
  d.setDate(d.getDate() + 7);
  const next = d.toISOString().slice(0, 10);
  await guard(async () => {
    await api.put(`/api/tasks/${task.id}`, { ...task, due_date: next });
    toast(`Moved to ${next}`, 'info');
    await load();
    await refreshBadge();
  });
}

async function remove(task) {
  if (!window.confirm(`Delete "${task.title}"?`)) return;
  await guard(async () => {
    await api.del(`/api/tasks/${task.id}`);
    toast('Task deleted', 'info');
    await load();
    await refreshBadge();
  });
}

function toggleCompleted() {
  showCompleted.value = !showCompleted.value;
  load();
}

function farmName(task) {
  return task.farm_name || farmsById.value.get(task.farm_id)?.name || 'Farm task';
}

onMounted(load);
</script>

<template>
  <div class="stack">
    <div class="row row--between">
      <h2 class="section-title" style="margin: 0">Tasks</h2>
      <div class="row" style="gap: 6px">
        <button class="btn btn--ghost btn--sm" @click="toggleCompleted">
          {{ showCompleted ? 'Hide completed' : 'Show completed' }}
        </button>
        <button class="btn btn--primary btn--sm" @click="showSchedule = true">Schedule a task</button>
      </div>
    </div>

    <div class="field">
      <label for="task-farm">Farm</label>
      <select id="task-farm" v-model="farmFilter" class="select">
        <option value="all">All farms</option>
        <option v-for="f in store.farms" :key="f.id" :value="f.id">{{ f.name }}</option>
      </select>
    </div>

    <div v-if="loading" class="skeleton" style="height: 260px" />

    <div v-else-if="!filtered.length" class="card">
      <div class="card__body">
        <div class="empty">
          <div class="empty__title">Nothing scheduled</div>
          Schedule insect scouting and crop inspection so nothing is missed during the season.
        </div>
      </div>
    </div>

    <template v-else>
      <section v-for="group in [
        ['overdue', 'Overdue', 'stripe-red'],
        ['today', 'Due today', 'stripe-amber'],
        ['week', 'Next 7 days', 'stripe-green'],
        ['later', 'Later', 'stripe-plain'],
        ['done', 'Completed', 'stripe-plain'],
      ]" :key="group[0]">
        <template v-if="buckets[group[0]].length">
          <h2 class="section-title">{{ group[1] }} ({{ buckets[group[0]].length }})</h2>
          <div class="card card__body--flush">
            <div v-for="task in buckets[group[0]]" :key="task.id" class="field-row" style="border: 0; border-radius: 0">
              <div class="field-row__stripe" :class="group[2]" />
              <div class="field-row__main" style="cursor: default">
                <div class="row row--between row--top">
                  <div class="grow">
                    <div class="field-row__name" style="font-size: 14px">{{ task.title }}</div>
                    <div class="field-row__meta">
                      {{ task.due_date }} · {{ farmName(task) }}
                      <template v-if="task.field_name">
                        · <RouterLink :to="`/fields/${task.field_id}`">{{ task.field_name }}</RouterLink>
                      </template>
                      <template v-if="task.repeat_interval !== 'none'">
                        · <span class="chip chip--plain">{{ task.repeat_interval }}</span>
                      </template>
                      <template v-if="task.completed_at"> · done {{ task.completed_at }}</template>
                    </div>
                    <div v-if="task.notes" class="tiny muted" style="margin-top: 2px">{{ task.notes }}</div>
                  </div>
                  <div class="row" style="gap: 6px; flex: 0 0 auto">
                    <button v-if="!task.completed_at" class="btn btn--sm" @click="complete(task)">Done</button>
                    <button v-if="!task.completed_at" class="btn btn--ghost btn--sm" @click="postpone(task)">+7d</button>
                    <button class="btn btn--ghost btn--sm" @click="remove(task)">Delete</button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </template>
      </section>
    </template>

    <TaskDialog
      v-if="showSchedule"
      :farms="store.farms"
      :fields="fields"
      @cancel="showSchedule = false"
      @saved="
        showSchedule = false;
        load();
        refreshBadge();
      "
    />
  </div>
</template>