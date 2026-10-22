<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { api } from '../api.js';
import { guard, refreshBadge, store, toast } from '../composables/store.js';
import { fmt } from '../composables/format.js';
import ModalDialog from '../components/ModalDialog.vue';

const route = useRoute();
const router = useRouter();

const fieldId = computed(() => Number(route.params.fieldId));
const detail = ref(null);
const budget = ref(null);
const loading = ref(true);
const crop = ref('');
const targetYield = ref(null);
const editing = ref(null);
const busy = ref(false);

const field = computed(() => detail.value?.field || null);
const plantings = computed(() => (detail.value?.entries || []).filter((e) => e.type === 'planting'));
const soilTests = computed(() => (detail.value?.entries || []).filter((e) => e.type === 'soil_test'));

const NUTRIENTS = computed(() => {
  const n = budget.value?.nutrients;
  console.log('NUTRIENTS', n);
  if (!n) return [];
  return [
    { key: 'n', label: 'N', ...n.n },
    { key: 'p', label: 'P₂O₅', ...n.p },
    { key: 'k', label: 'K₂O', ...n.k },
  ];
});

async function load() {
  loading.value = true;
  await guard(async () => {
    const params = new URLSearchParams();
    if (crop.value) params.set('crop', crop.value);
    if (targetYield.value) params.set('targetYield', String(targetYield.value));
    const [d, b] = await Promise.all([
      api.get(`/api/fields/${fieldId.value}`),
      api.get(`/api/fields/${fieldId.value}/nutrient-budget?${params.toString()}`),
    ]);
    detail.value = d;
    budget.value = b;
  });
  loading.value = false;
}

function newSeason() {
  editing.value = {
    farm_id: field.value?.farm_id,
    field_id: fieldId.value,
    season_year: new Date().getFullYear(),
    crop: crop.value || plantings.value[0]?.data.crop || '',
    variety: '',
    target_yield: targetYield.value || null,
    notes: '',
  };
}

async function save() {
  busy.value = true;
  await guard(async () => {
    await api.post(`/api/fields/${fieldId.value}/seasons`, editing.value);
    toast('Season saved', 'info');
    busy.value = false;
    editing.value = null;
    await load();
  });
  busy.value = false;
}

async function removeSeason(s) {
  if (!window.confirm(`Delete the ${s.crop || ''} ${s.season_year} season record?`)) return;
  await guard(async () => {
    await api.del(`/api/seasons/${s.id}`);
    toast('Season deleted', 'info');
    await load();
  });
}

onMounted(load);
</script>

<template>
  <div class="stack">
    <div class="page-head">
      <button class="btn btn--ghost btn--sm" @click="router.push(`/fields/${fieldId}`)">‹ {{ field?.name || 'Field' }}</button>
      <h1 style="font-size: 20px">Rotation plan - {{ field?.name }}</h1>
      <div class="small muted">
        {{ field?.farm_name }}
        <template v-if="field?.area_ha != null"> · {{ fmt(field.area_ha) }} ha</template>
      </div>
    </div>

    <div v-if="loading" class="skeleton" style="height: 260px" />

    <template v-else-if="field">
      <div class="card">
        <div class="card__head">Budget for</div>
        <div class="card__body">
          <div class="grid-3">
            <div class="field">
              <label>Crop</label>
              <input
                v-model="crop"
                class="input"
                list="crop-options"
                placeholder="Wheat"
                @change="load"
              />
              <datalist id="crop-options">
                <option v-for="c in store.reference.crops" :key="c.crop" :value="c.crop" />
              </datalist>
            </div>
            <div class="field">
              <label>Target yield (t/ha)</label>
              <input
                v-model.number="targetYield"
                class="input"
                type="number"
                step="0.1"
                inputmode="decimal"
                placeholder="3.5"
                @change="load"
              />
            </div>
            <div class="field" style="justify-content: flex-end">
              <button class="btn btn--primary" :disabled="!crop" @click="load">Recalculate</button>
            </div>
          </div>

          <div v-if="budget && !budget.cropKnown && crop" class="alert alert--amber">
            <span class="alert__icon">i</span>
            <div>
              "{{ crop }}" is not in the crop reference, so no uptake figures are available. The
              fertiliser inputs below are still recorded but demand cannot be calculated.
            </div>
          </div>

          <table v-if="budget" class="table">
            <thead>
              <tr>
                <th>Element</th>
                <th class="num">Required</th>
                <th class="num">Soil supply</th>
                <th class="num">Fertiliser</th>
                <th class="num">Balance</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in NUTRIENTS" :key="row.key">
                <td>{{ row.label }}</td>
                <td class="num">
                  {{ row.required === null ? '-' : `${fmt(row.required)} kg/ha` }}
                  <span v-if="row.uptakePerTonne" class="tiny muted">
                    ({{ fmt(row.uptakePerTonne) }}/t)
                  </span>
                </td>
                <td class="num">{{ fmt(row.soilAvailable) }}</td>
                <td class="num">{{ fmt(row.appliedFertiliser) }}</td>
                <td class="num" :class="row.balance > 0 ? 'text-red' : row.balance !== null ? 'text-green' : ''">
                  <template v-if="row.balance === null">-</template>
                  <template v-else>{{ row.balance > 0 ? '-' : '+' }}{{ fmt(Math.abs(row.balance)) }}</template>
                </td>
              </tr>
            </tbody>
          </table>

          <div v-if="budget?.fertiliser?.lines?.length" class="stack stack--tight" style="margin-top: 10px">
            <span class="label">Fertiliser inputs</span>
            <ul class="kv">
              <li v-for="(line, i) in budget.fertiliser.lines" :key="i">
                <span>
                  {{ line.date }} · {{ line.product }}
                  <span v-if="!line.known" class="tiny muted">(unknown analysis)</span>
                </span>
                <strong>{{ fmt(line.n) }} kg N/ha</strong>
              </li>
            </ul>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card__head">Planting history</div>
        <div class="card__body card__body--flush">
          <div v-if="!plantings.length" class="empty">
            <div class="empty__title">No plantings logged</div>
            Add a planting entry on the field timeline to start the rotation.
          </div>
          <div v-for="p in plantings" :key="p.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe stripe-green" />
            <RouterLink class="field-row__main" :to="`/fields/${fieldId}`">
              <div class="row row--between">
                <span class="field-row__name">{{ p.data.crop }}</span>
                <span class="tiny muted">{{ p.entry_date }}</span>
              </div>
              <div class="field-row__meta">
                {{ p.data.variety || 'no variety' }}
                <template v-if="p.data.rateKgHa"> · {{ fmt(p.data.rateKgHa) }} kg/ha</template>
                <template v-if="p.data.rateSeedsM2"> · {{ fmt(p.data.rateSeedsM2) }} seeds/m²</template>
              </div>
            </RouterLink>
          </div>
        </div>
      </div>

      <div v-if="detail.seasons.length" class="card">
        <div class="card__head">Season plan</div>
        <div class="card__body card__body--flush">
          <div v-for="s in detail.seasons" :key="s.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe stripe-green" />
            <div class="field-row__main" style="cursor: default">
              <div class="row row--between">
                <span class="field-row__name">{{ s.crop || 'Unspecified crop' }} · {{ s.season_year }}</span>
                <button class="btn btn--ghost btn--sm" @click="removeSeason(s)">Delete</button>
              </div>
              <div class="field-row__meta">
                <template v-if="s.variety">{{ s.variety }} · </template>
                <template v-if="s.target_yield">{{ fmt(s.target_yield) }} t/ha · </template>
                {{ s.notes || 'no notes' }}
              </div>
            </div>
          </div>
        </div>
        <div class="card__foot">
          <button class="btn btn--sm btn--block" @click="newSeason">Add a season to the plan</button>
        </div>
      </div>

      <div v-if="soilTests.length" class="card">
        <div class="card__head">Soil test history</div>
        <div class="card__body card__body--flush">
          <div v-for="s in soilTests" :key="s.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe stripe-green" />
            <RouterLink class="field-row__main" :to="`/fields/${fieldId}`">
              <div class="row row--between">
                <span class="field-row__name">{{ s.data.lab || 'Soil test' }}</span>
                <span class="tiny muted">{{ s.entry_date }}</span>
              </div>
              <div class="field-row__meta">
                <template v-if="s.data.ph != null">pH {{ s.data.ph }} · </template>
                <template v-if="s.data.n != null">N {{ s.data.n }} · </template>
                <template v-if="s.data.p != null">P {{ s.data.p }} · </template>
                <template v-if="s.data.k != null">K {{ s.data.k }} · </template>
                <template v-if="s.data.organicMatter != null">OM {{ s.data.organicMatter }}%</template>
              </div>
            </RouterLink>
          </div>
        </div>
      </div>

      <ModalDialog
        v-if="editing"
        title="Add season"
        confirm-label="Save"
        :busy="busy"
        @cancel="editing = null"
        @confirm="save"
      >
        <div class="stack">
          <div class="grid-2">
            <div class="field">
              <label>Season year</label>
              <input v-model.number="editing.season_year" class="input" type="number" step="1" inputmode="numeric" />
            </div>
            <div class="field">
              <label>Crop</label>
              <input v-model="editing.crop" class="input" list="crop-options" />
            </div>
          </div>
          <div class="grid-2">
            <div class="field">
              <label>Variety</label>
              <input v-model="editing.variety" class="input" />
            </div>
            <div class="field">
              <label>Target yield (t/ha)</label>
              <input v-model.number="editing.target_yield" class="input" type="number" step="0.1" inputmode="decimal" />
            </div>
          </div>
          <div class="field">
            <label>Notes</label>
            <textarea v-model="editing.notes" class="textarea" placeholder="Rotation intent, cover crop, planned herbicide program" />
          </div>
        </div>
      </ModalDialog>
    </template>
  </div>
</template>