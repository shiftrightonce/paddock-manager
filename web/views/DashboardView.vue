<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { api } from '../api.js';
import { guard, loadFarms, refreshBadge, setCurrentFarm, store, toast } from '../composables/store.js';
import { fmt } from '../composables/format.js';
import NewFarmDialog from '../components/NewFarmDialog.vue';
import TaskDialog from '../components/TaskDialog.vue';

const router = useRouter();
const data = ref(null);
const loading = ref(true);
const showSchedule = ref(false);
const showNewFarm = ref(false);
const fields = ref([]);
const rollup = computed(() => data.value);
const currentFarm = computed(() => store.farms.find((f) => f.id === store.currentFarmId) || null);

async function load() {
  loading.value = true;
  await guard(async () => {
    if (!store.currentFarmId) await loadFarms();
    if (!store.currentFarmId) {
      data.value = null;
      return;
    }
    const [dash, farmFields] = await Promise.all([
      api.get(`/api/dashboard?farmId=${store.currentFarmId}`),
      api.get(`/api/fields?farmId=${store.currentFarmId}`),
    ]);
    data.value = dash;
    fields.value = farmFields;
  });
  loading.value = false;
}

async function switchFarm(id) {
  await setCurrentFarm(id);
  await load();
  await refreshBadge();
}

async function completeTask(task) {
  await guard(async () => {
    const result = await api.post(`/api/tasks/${task.id}/complete`);
    if (result.entry) toast(`Logged an inspection entry on ${task.field_name}`, 'info');
    if (result.rescheduledTo) toast(`Rescheduled to ${result.rescheduledTo}`, 'info');
    await load();
    await refreshBadge();
  });
}

function goField(fieldId) {
  router.push(`/fields/${fieldId}`);
}

onMounted(load);
</script>

<template>
  <div class="stack">
    <div v-if="!store.farms.length" class="card">
      <div class="card__body">
        <div class="empty">
          <div class="empty__title">No farms yet</div>
          Add your first farm, then add fields under it.
          <div style="margin-top: 12px">
            <button class="btn btn--primary" @click="showNewFarm = true">Add a farm</button>
          </div>
        </div>
      </div>
    </div>

    <template v-else>
      <div class="row row--between">
        <div class="field grow">
          <label for="farm-switcher">Farm</label>
          <select
            id="farm-switcher"
            class="select"
            :value="store.currentFarmId"
            @change="switchFarm(Number($event.target.value))"
          >
            <option v-for="f in store.farms" :key="f.id" :value="f.id">
              {{ f.name }} ({{ f.fieldCount }} fields)
            </option>
          </select>
        </div>
      </div>

      <div v-if="currentFarm" class="card">
        <div class="card__body">
          <div class="row row--between row--top">
            <div>
              <h2 style="font-size: 17px">{{ currentFarm.name }}</h2>
              <div class="small muted">{{ currentFarm.location || 'No location set' }}</div>
              <div v-if="currentFarm.weather_station" class="tiny muted">
                Weather station: {{ currentFarm.weather_station }} (informational - rainfall is entered manually)
              </div>
            </div>
            <div class="stat-grid" style="flex: 0 0 auto">
              <div class="stat stat--red">
                <div class="stat__label">Red</div>
                <div class="stat__value">{{ currentFarm.redCount }}</div>
              </div>
              <div class="stat stat--amber">
                <div class="stat__label">Amber</div>
                <div class="stat__value">{{ currentFarm.amberCount }}</div>
              </div>
            </div>
          </div>
        </div>
        <div class="link-list">
          <RouterLink to="/export">Export summary<span class="link-list__chev">›</span></RouterLink>
          <RouterLink to="/farms">Manage farms and fields<span class="link-list__chev">›</span></RouterLink>
          <RouterLink to="/mixes">Tank mixes and compatibility<span class="link-list__chev">›</span></RouterLink>
        </div>
      </div>

      <div v-if="loading" class="stack">
        <div class="skeleton" />
        <div class="skeleton" style="height: 150px" />
      </div>

      <template v-else-if="rollup">
        <!-- needs attention, cross-farm -->
        <h2 class="section-title">Needs attention (all farms)</h2>
        <div class="card">
          <div v-if="!rollup.attention.length" class="card__body">
            <div class="empty">
              <div class="empty__title">Nothing flagged</div>
              No resistance conflicts, active residuals or overdue tasks across your farms.
            </div>
          </div>
          <div v-else class="link-list">
            <button
              v-for="(item, i) in rollup.attention"
              :key="`${item.kind}-${i}`"
              type="button"
              @click="item.fieldId && goField(item.fieldId)"
            >
              <span
                :style="{
                  width: '8px',
                  height: '8px',
                  borderRadius: '50%',
                  background: item.level === 'red' ? 'var(--red)' : 'var(--amber)',
                  flex: '0 0 auto',
                }"
              />
              <span class="grow">
                <span class="strong small">{{ item.text }}</span>
                <span class="tiny muted" style="display: block">
                  {{ item.farmName }}<template v-if="item.fieldName"> / {{ item.fieldName }}</template>
                  <template v-if="item.date"> · {{ item.date }}</template>
                </span>
              </span>
              <span v-if="item.fieldId" class="link-list__chev">›</span>
            </button>
          </div>
        </div>

        <!-- fields -->
        <h2 class="section-title">Fields</h2>
        <div class="card card__body--flush">
          <div v-if="!rollup.currentFarm.fields.length" class="empty">
            <div class="empty__title">No fields on this farm</div>
            Add fields under Farms.
          </div>
          <div v-for="field in rollup.currentFarm.fields" :key="field.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe" :class="`stripe-${field.status}`" />
            <button class="field-row__main" type="button" @click="goField(field.id)">
              <div class="row row--between">
                <span class="field-row__name">{{ field.name }}</span>
                <span class="chip" :class="`chip--${field.status}`">
                  {{ field.status === 'red' ? 'Resistance' : field.status === 'amber' ? 'Residual' : 'Clear' }}
                </span>
              </div>
              <div class="field-row__meta">
                <template v-if="field.area_ha != null">{{ fmt(field.area_ha) }} ha · </template>
                <template v-if="field.soil_type">{{ field.soil_type }}</template>
                <template v-if="!field.area_ha && !field.soil_type">No area or soil type recorded</template>
              </div>
              <div v-if="field.resistanceCount" class="field-row__flags">
                <span class="tiny" style="color: var(--red)">
                  {{ field.resistanceCount }} resistance record{{ field.resistanceCount === 1 ? '' : 's' }}
                </span>
              </div>
              <div v-else-if="field.activeResidualDays != null" class="field-row__meta">
                Residual applied {{ field.activeResidualDays }}d ago
                ({{ field.lastResidual?.residuals?.join(', ') }})
              </div>
            </button>
            <div class="field-row__chev">›</div>
          </div>
        </div>

        <!-- upcoming tasks -->
        <h2 class="section-title">Upcoming tasks</h2>
        <div class="card card__body--flush">
          <div v-if="!rollup.upcoming.length" class="empty">
            <div class="empty__title">Nothing scheduled</div>
            Schedule scouting or inspection tasks to keep ahead of the season.
          </div>
          <template v-else>
            <div v-for="task in rollup.upcoming" :key="task.id" class="field-row" style="border: 0; border-radius: 0">
              <div class="field-row__stripe" :class="task.overdue ? 'stripe-red' : 'stripe-green'" />
              <div class="field-row__main" style="cursor: default">
                <div class="row row--between">
                  <span class="field-row__name" style="font-size: 14px">{{ task.title }}</span>
                  <button class="btn btn--sm" type="button" @click="completeTask(task)">Done</button>
                </div>
                <div class="field-row__meta">
                  {{ task.due_date }}
                  <span v-if="task.overdue" class="chip chip--red" style="margin-left: 6px">Overdue</span>
                  <span v-if="task.field_name" class="chip chip--plain" style="margin-left: 6px">{{ task.field_name }}</span>
                  <span v-if="task.repeat_interval !== 'none'" class="chip chip--plain" style="margin-left: 6px">
                    {{ task.repeat_interval }}
                  </span>
                </div>
              </div>
            </div>
          </template>
          <div class="card__foot">
            <button class="btn btn--sm btn--block" @click="showSchedule = true">Schedule a task</button>
          </div>
        </div>
      </template>
    </template>

    <TaskDialog
      v-if="showSchedule"
      :farms="store.farms"
      :fields="fields"
      :preset-farm-id="store.currentFarmId"
      @cancel="showSchedule = false"
      @saved="
        showSchedule = false;
        load();
        refreshBadge();
      "
    />

    <NewFarmDialog v-if="showNewFarm" @cancel="showNewFarm = false" @saved="showNewFarm = false; load()" />
  </div>
</template>