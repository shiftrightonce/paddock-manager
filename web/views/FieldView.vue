<script setup>
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { api, download, links } from '../api.js';
import { guard, refreshBadge, store, toast } from '../composables/store.js';
import { fmt, IRRIGATION_LABEL } from '../composables/format.js';
import EntryDialog from '../components/EntryDialog.vue';
import PhotoGrid from '../components/PhotoGrid.vue';
import StatusStripe from '../components/StatusStripe.vue';

const route = useRoute();
const router = useRouter();

const fieldId = computed(() => Number(route.params.fieldId));
const detail = ref(null);
const budget = ref(null);
const moa = ref([]);
const mixes = ref([]);
const loading = ref(true);
const tab = ref('overview');
const showEntry = ref(false);
const editEntry = ref(null);
const typeFilter = ref('');

const field = computed(() => detail.value?.field || null);
const farm = computed(() => detail.value?.farm || null);
const entries = computed(() => detail.value?.entries || []);
const water = computed(() => detail.value?.water || null);
const resistance = computed(() => detail.value?.resistance || []);
const lastResidual = computed(() => detail.value?.lastResidual || null);
const options = computed(() => store.reference.options);
const chemicals = computed(() => store.reference.chemicals || []);

const visibleEntries = computed(() =>
  typeFilter.value ? entries.value.filter((e) => e.type === typeFilter.value) : entries.value,
);

async function load() {
  loading.value = true;
  await guard(async () => {
    const id = fieldId.value;
    const [d, b, m, mixList] = await Promise.all([
      api.get(`/api/fields/${id}`),
      api.get(`/api/fields/${id}/nutrient-budget`),
      api.get(`/api/fields/${id}/moa-history`),
      api.get('/api/mixes'),
    ]);
    detail.value = d;
    budget.value = b;
    moa.value = m;
    mixes.value = mixList;
  });
  loading.value = false;
}

async function afterSave() {
  showEntry.value = false;
  editEntry.value = null;
  await load();
  await refreshBadge();
}

async function recordResistance(product, moaGroup, event) {
  const species = window.prompt(`Which weed failed on ${product}?`);
  if (!species) {
    event.target.checked = false;
    return;
  }
  await guard(async () => {
    await api.post(`/api/fields/${fieldId.value}/resistance`, {
      weed_species: species.trim(),
      product,
      moa_group: moaGroup || '',
      source: 'operator',
      confirmed_on: new Date().toISOString().slice(0, 10),
    });
    toast(`Recorded resistance on ${field.value.name}`, 'info');
    event.target.checked = false;
    await load();
    await refreshBadge();
  });
}

async function confirmResistance(rec) {
  const today = new Date().toISOString().slice(0, 10);
  await guard(async () => {
    await api.put(`/api/resistance/${rec.id}`, { ...rec, confirmed_on: today });
    toast('Marked as confirmed', 'info');
    await load();
    await refreshBadge();
  });
}

async function deleteResistance(rec) {
  if (!window.confirm('Delete this resistance record?')) return;
  await guard(async () => {
    await api.del(`/api/resistance/${rec.id}`);
    toast('Record deleted', 'info');
    await load();
    await refreshBadge();
  });
}

function openEntry(entry) {
  editEntry.value = entry;
  showEntry.value = true;
}

function addEntry(type) {
  editEntry.value = type ? { type } : null;
  showEntry.value = true;
}

async function openBom() {
  await guard(async () => {
    window.open(await links.bom(farm.value?.location), '_blank', 'noopener');
  });
}

const ENTRY_ICON = {
  planting: '🌱',
  chemical_application: '💧',
  weed_scouting: '🌿',
  soil_test: '🧪',
  soil_moisture: '💦',
  harvest: '🚜',
  water: '🌧',
  inspection: '🔍',
};

const entryLabel = (type) => options.value.entryTypes?.find((t) => t.type === type)?.label || type;

function entryDetail(entry) {
  const d = entry.data || {};
  switch (entry.type) {
    case 'planting':
      return [
        d.crop,
        d.variety,
        d.rateKgHa ? `${fmt(d.rateKgHa)} kg/ha` : null,
        d.rateSeedsM2 ? `${fmt(d.rateSeedsM2)} seeds/m²` : null,
        d.seedsPerKg ? `${fmt(d.seedsPerKg, 0)} seeds/kg` : null,
        d.fertilisers?.length ? `${d.fertilisers.length} fertiliser product${d.fertilisers.length === 1 ? '' : 's'}` : null,
      ]
        .filter(Boolean)
        .join(' · ');
    case 'chemical_application':
      return [
        d.target,
        (d.products || []).map((p) => `${p.product} ${p.rate}${p.rateUnit}`).join(', '),
      ]
        .filter(Boolean)
        .join(' · ');
    case 'soil_test': {
      const bits = [];
      if (d.lab) bits.push(d.lab);
      if (d.ph != null) bits.push(`pH ${d.ph}`);
      if (d.n != null) bits.push(`N ${d.n} mg/kg`);
      if (d.organicMatter != null) bits.push(`OM ${d.organicMatter}%`);
      return bits.join(' · ');
    }
    case 'soil_moisture':
      return [
        d.reading != null ? `${fmt(d.reading)} ${d.unit || ''}` : null,
        d.depthCm ? `${d.depthCm} cm` : null,
        d.method,
      ]
        .filter(Boolean)
        .join(' · ');
    case 'weed_scouting':
      return [d.species, d.density, d.location].filter(Boolean).join(' · ');
    case 'water':
      return `${fmt(d.rainfallMm || 0)} mm rain + ${fmt(d.irrigationMm || 0)} mm irrigation${
        d.method && d.method !== 'none' ? ` (${IRRIGATION_LABEL[d.method] || d.method})` : ''
      }`;
    case 'harvest':
      return [
        d.yieldValue != null ? `${fmt(d.yieldValue)} ${d.yieldUnit || ''}` : null,
        d.moisture != null ? `${d.moisture}% moisture` : null,
      ]
        .filter(Boolean)
        .join(' · ');
    case 'inspection':
      return d.outcome || d.taskType || '';
    default:
      return '';
  }
}

const NUTRIENT_ROWS = computed(() => {
  const n = budget.value?.nutrients;
  if (!n) return [];
  return [
    { key: 'n', label: 'Nitrogen (N)', ...n.n },
    { key: 'p', label: 'Phosphorus (P₂O₅)', ...n.p },
    { key: 'k', label: 'Potassium (K₂O)', ...n.k },
  ].filter((row) => row.required !== null || row.soilAvailable !== null || row.appliedFertiliser > 0);
});

function barWidth(row) {
  if (!row.required) return 0;
  return Math.max(0, Math.min(100, Math.round((row.available / row.required) * 100)));
}

watch(fieldId, load);
onMounted(load);
</script>

<template>
  <div class="stack">
    <div v-if="loading" class="stack">
      <div class="skeleton" style="height: 120px" />
      <div class="skeleton" style="height: 260px" />
    </div>

    <template v-else-if="field">
      <div class="page-head">
        <button class="btn btn--ghost btn--sm" @click="router.push('/')">‹ Farms</button>
        <div class="row row--between row--top">
          <div>
            <h1 style="font-size: 20px">{{ field.name }}</h1>
            <div class="small muted">
              {{ field.farm_name }}
              <template v-if="field.area_ha != null"> · {{ fmt(field.area_ha) }} ha</template>
              <template v-if="field.soil_type"> · {{ field.soil_type }}</template>
            </div>
          </div>
          <div class="row" style="gap: 6px">
            <RouterLink class="btn btn--sm" to="/export">Export</RouterLink>
          </div>
        </div>
        <StatusStripe :status="field.status" />
        <ul class="reasons">
          <li v-for="(reason, i) in field.statusReasons" :key="i" :class="`reason reason--${reason.level}`">
            {{ reason.text }}
          </li>
        </ul>
      </div>

      <div v-if="field.status === 'red'" class="alert alert--red">
        <span class="alert__icon">!</span>
        <div>
          <strong>Resistance recorded on this field.</strong>
          Choose an alternative MOA group. Check the label for the plantback interval and any other
          restrictions before spraying.
        </div>
      </div>
      <div v-else-if="field.status === 'amber' && lastResidual" class="alert alert--amber">
        <span class="alert__icon">i</span>
        <div>
          <strong>Residual may still be active.</strong>
          {{ lastResidual.residuals.join(', ') }} applied {{ lastResidual.daysSince }} days ago
          ({{ lastResidual.date }}). Check label plantback intervals before sowing. No automatic
          plantback calculation is applied.
        </div>
      </div>

      <div class="tabs" role="tablist">
        <button
          v-for="t in [['overview', 'Overview'], ['timeline', 'Timeline'], ['resistance', 'Resistance'], ['rotation', 'Rotation'], ['water', 'Water']]"
          :key="t[0]"
          role="tab"
          :class="{ 'is-on': tab === t[0] }"
          :aria-selected="tab === t[0]"
          @click="tab = t[0]"
        >
          {{ t[1] }}
        </button>
      </div>

      <!-- OVERVIEW -->
      <template v-if="tab === 'overview'">
        <div class="grid-2">
          <div class="card">
            <div class="card__head">Water - last 7 days</div>
            <div class="card__body">
              <div class="stat" :class="(water?.totalMm || 0) >= 25 ? 'stat--green' : 'stat--amber'">
                <div class="stat__label">Rainfall + irrigation</div>
                <div class="stat__value">{{ fmt(water?.totalMm || 0) }}<span class="stat__unit">mm</span></div>
              </div>
              <ul class="kv">
                <li><span>Rainfall</span><strong>{{ fmt(water?.rainfallMm || 0) }} mm</strong></li>
                <li><span>Irrigation</span><strong>{{ fmt(water?.irrigationMm || 0) }} mm</strong></li>
              </ul>
              <div class="row" style="gap: 6px; margin-top: 8px">
                <button class="btn btn--sm" @click="addEntry('water')">Log water</button>
                <button class="btn btn--ghost btn--sm" @click="openBom">BOM rainfall</button>
              </div>
            </div>
          </div>

          <div class="card">
            <div class="card__head">Nutrient budget</div>
            <div class="card__body">
              <div v-if="!NUTRIENT_ROWS.length" class="empty">
                <div class="empty__title">No soil test or planting logged</div>
                Add a soil test and a planting entry to see supply against crop demand.
              </div>
              <template v-else>
                <div class="tiny muted" style="margin-bottom: 8px">
                  {{ budget.cropKnown ? budget.crop : 'Crop not set' }}
                  <template v-if="budget.targetYield"> · target {{ fmt(budget.targetYield) }} t/ha</template>
                </div>
                <div v-for="row in NUTRIENT_ROWS" :key="row.key" class="bar-row">
                  <span class="bar-row__label">{{ row.label }}</span>
                  <span class="bar-row__track">
                    <span
                      class="bar-row__fill"
                      :class="row.balance > 0 ? 'is-deficit' : 'is-ok'"
                      :style="{ width: `${barWidth(row)}%` }"
                    />
                  </span>
                  <span class="bar-row__value" :class="row.balance > 0 ? 'text-red' : 'text-green'">
                    <template v-if="row.balance === null">-</template>
                    <template v-else-if="row.balance > 0">−{{ fmt(row.balance) }}</template>
                    <template v-else>+{{ fmt(Math.abs(row.balance)) }}</template>
                  </span>
                </div>
                <ul class="kv tiny">
                  <li><span>Soil supply</span><strong>{{ fmt(budget.soilTest?.n || 0) }} kg N/ha</strong></li>
                  <li><span>Fertiliser applied</span><strong>{{ fmt(budget.fertiliser.n) }} kg N/ha</strong></li>
                </ul>
                <div class="tiny muted">
                  Supply = latest soil test + fertiliser inputs. Demand = crop uptake × target yield.
                </div>
              </template>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card__head">Recent activity</div>
          <div class="card__body card__body--flush">
            <div v-if="!entries.length" class="empty">
              <div class="empty__title">Nothing logged yet</div>
              Add your first entry below.
            </div>
            <div v-for="entry in entries.slice(0, 5)" :key="entry.id" class="field-row" style="border: 0; border-radius: 0">
              <div class="field-row__stripe stripe-green" />
              <button class="field-row__main" type="button" @click="openEntry(entry)">
                <div class="row row--between">
                  <span class="field-row__name" style="font-size: 14px">
                    {{ ENTRY_ICON[entry.type] }} {{ entryLabel(entry.type) }}
                  </span>
                  <span class="tiny muted">{{ entry.entry_date }}</span>
                </div>
                <div class="field-row__meta">{{ entryDetail(entry) || entry.notes || 'No detail' }}</div>
              </button>
              <div class="field-row__chev">›</div>
            </div>
          </div>
          <div v-if="entries.length > 5" class="card__foot">
            <button class="btn btn--ghost btn--sm btn--block" @click="tab = 'timeline'">
              View full timeline ({{ entries.length }})
            </button>
          </div>
        </div>

        <div class="card">
          <div class="card__head">MOA groups used (12 months)</div>
          <div class="card__body">
            <div v-if="!moa.length" class="tiny muted">No chemical applications logged.</div>
            <div v-else class="row row--wrap" style="gap: 6px">
              <span
                v-for="g in moa"
                :key="g.group"
                class="chip"
                :class="g.resistance ? 'chip--red' : 'chip--plain'"
                :title="g.description"
              >
                Group {{ g.group }} · {{ g.applications }}
              </span>
            </div>
            <div v-if="moa.some((g) => g.resistance)" class="alert alert--red" style="margin-top: 10px">
              <span class="alert__icon">!</span>
              <div>Red groups have a recorded resistance failure on this field. Avoid them.</div>
            </div>
          </div>
        </div>
      </template>

      <!-- TIMELINE -->
      <template v-else-if="tab === 'timeline'">
        <div class="row" style="gap: 6px; flex-wrap: wrap">
          <button class="btn btn--primary btn--sm" @click="addEntry()">Add entry</button>
          <button
            v-for="t in options.entryTypes || []"
            :key="t.type"
            class="btn btn--sm"
            @click="addEntry(t.type)"
          >
            + {{ t.label }}
          </button>
        </div>

        <div class="row row--wrap" style="gap: 6px">
          <button class="chip chip--button" :class="{ 'chip--plain': typeFilter }" @click="typeFilter = ''">
            All
          </button>
          <button
            v-for="t in options.entryTypes || []"
            :key="t.type"
            class="chip chip--button"
            :class="{ 'chip--plain': typeFilter !== t.type }"
            @click="typeFilter = t.type"
          >
            {{ t.label }}
          </button>
        </div>

        <div v-if="!visibleEntries.length" class="card">
          <div class="card__body">
            <div class="empty">
              <div class="empty__title">No matching entries</div>
              Record plantings, sprays, scouting observations, soil tests, water and harvests.
            </div>
          </div>
        </div>

        <div v-for="entry in visibleEntries" :key="entry.id" class="card">
          <div class="card__body">
            <div class="row row--between row--top">
              <div>
                <div class="strong">{{ ENTRY_ICON[entry.type] }} {{ entry.title || entryLabel(entry.type) }}</div>
                <div class="tiny muted">{{ entry.entry_date }}</div>
              </div>
              <button class="btn btn--ghost btn--sm" @click="openEntry(entry)">Edit</button>
            </div>

            <div class="small" style="margin-top: 6px">{{ entryDetail(entry) }}</div>
            <div v-if="entry.notes" class="tiny muted" style="margin-top: 4px">{{ entry.notes }}</div>

            <div v-if="entry.type === 'chemical_application'" class="product-list">
              <div v-for="(p, i) in entry.data.products || []" :key="i" class="product-row">
                <span class="product-row__name">{{ p.product }}</span>
                <span class="chip chip--plain">MOA {{ p.moa || '?' }}</span>
                <span v-if="p.residual" class="chip chip--amber">Residual</span>
                <span class="tiny muted">{{ p.rate }} {{ p.rateUnit }}<template v-if="p.active"> · {{ p.active }}</template></span>
              </div>
              <div v-if="entry.hasConflict" class="alert alert--red" style="margin-top: 8px">
                <span class="alert__icon">!</span>
                <div>
                  <strong>Resistance conflict at time of application.</strong>
                  <div v-for="c in entry.conflicts.filter((x) => x.conflict)" :key="c.product" class="small">
                    {{ c.product }}:
                    <span v-for="m in c.matches" :key="m.id">
                      {{ m.weedSpecies }} resistant ({{ m.matchedOn.join(' + ') }})
                    </span>
                  </div>
                  <div class="tiny muted">Saved as a record. Nothing is blocked.</div>
                </div>
              </div>
            </div>

            <PhotoGrid :entry-id="entry.id" :photos="entry.photos" @changed="load" />
          </div>
        </div>
      </template>

      <!-- RESISTANCE -->
      <template v-else-if="tab === 'resistance'">
        <div class="card">
          <div class="card__head">Recorded resistance</div>
          <div class="card__body">
            <div v-if="!resistance.length" class="empty">
              <div class="empty__title">No resistance recorded</div>
              Tick a product below to record a confirmed failure.
            </div>
            <div v-else class="link-list">
              <div v-for="rec in resistance" :key="rec.id" class="field-row" style="border: 0">
                <div class="field-row__stripe stripe-red" />
                <div class="field-row__main" style="cursor: default">
                  <div class="row row--between">
                    <span class="field-row__name" style="font-size: 14px">
                      {{ rec.weed_species }}
                      <span class="chip chip--red">{{ rec.product || 'unspecified' }}</span>
                      <span v-if="rec.moa_group" class="chip chip--plain">MOA {{ rec.moa_group }}</span>
                    </span>
                    <div class="row" style="gap: 6px">
                      <button v-if="!rec.confirmed_on" class="btn btn--sm" @click="confirmResistance(rec)">
                        Confirm
                      </button>
                      <button class="btn btn--ghost btn--sm" @click="deleteResistance(rec)">Delete</button>
                    </div>
                  </div>
                  <div class="field-row__meta">
                    <template v-if="rec.active">Active {{ rec.active }} · </template>
                    <template v-if="rec.confirmed_on">confirmed {{ rec.confirmed_on }} · </template>
                    <template v-else>unconfirmed · </template>
                    {{ rec.source || 'no source recorded' }}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card__head">Record a resistance failure</div>
          <div class="card__body">
            <p class="small muted" style="margin-top: 0">
              Select a product to record a confirmed failure. You will be asked for the weed species.
              Recording resistance turns this field red.
            </p>
            <div v-if="!chemicals.length" class="tiny muted">No chemical reference loaded.</div>
            <div class="check-list">
              <label v-for="c in chemicals" :key="c.product" class="check-row">
                <input
                  type="checkbox"
                  :checked="resistance.some((r) => r.product === c.product)"
                  :disabled="resistance.some((r) => r.product === c.product)"
                  @change="recordResistance(c.product, c.moa, $event)"
                />
                <span class="grow">
                  <span class="strong small">{{ c.product }}</span>
                  <span class="tiny muted" style="display: block">
                    {{ c.active }} · MOA {{ c.moa }}<template v-if="c.residual"> · residual</template>
                  </span>
                </span>
              </label>
            </div>
          </div>
        </div>

        <RouterLink class="btn btn--block" :to="`/fields/${fieldId}/resistance`">
          Open full resistance history
        </RouterLink>
      </template>

      <!-- ROTATION -->
      <template v-else-if="tab === 'rotation'">
        <div class="card">
          <div class="card__head">Planting and fertiliser history</div>
          <div class="card__body card__body--flush">
            <div v-if="!entries.filter((e) => e.type === 'planting').length" class="empty">
              <div class="empty__title">No plantings recorded</div>
              Log a planting entry to start the rotation history.
            </div>
            <div
              v-for="plant in entries.filter((e) => e.type === 'planting')"
              :key="plant.id"
              class="field-row"
              style="border: 0; border-radius: 0"
            >
              <div class="field-row__stripe stripe-green" />
              <button class="field-row__main" type="button" @click="openEntry(plant)">
                <div class="row row--between">
                  <span class="field-row__name">{{ plant.data.crop }}</span>
                  <span class="chip chip--plain">{{ plant.data.variety || 'no variety' }}</span>
                </div>
                <div class="field-row__meta">
                  {{ plant.entry_date }}
                  <template v-if="plant.data.rateKgHa"> · {{ fmt(plant.data.rateKgHa) }} kg/ha</template>
                  <template v-if="plant.data.rateSeedsM2"> · {{ fmt(plant.data.rateSeedsM2) }} seeds/m²</template>
                </div>
                <div v-if="plant.fertiliserCount" class="tiny muted">
                  {{ plant.fertiliserCount }} fertiliser product{{ plant.fertiliserCount === 1 ? '' : 's' }} ·
                  {{ fmt(plant.fertiliserContribution.n) }} kg N/ha
                </div>
              </button>
              <div class="field-row__chev">›</div>
            </div>
          </div>
        </div>

        <div v-if="detail.seasons.length" class="card">
          <div class="card__head">Season history</div>
          <div class="card__body card__body--flush">
            <div v-for="s in detail.seasons" :key="s.id" class="field-row" style="border: 0; border-radius: 0">
              <div class="field-row__stripe stripe-green" />
              <div class="field-row__main" style="cursor: default">
                <div class="field-row__name">{{ s.crop }} · {{ s.season_year }}</div>
                <div class="field-row__meta">
                  <template v-if="s.variety">{{ s.variety }} · </template>
                  <template v-if="s.target_yield">{{ fmt(s.target_yield) }} t/ha · </template>
                  <template v-if="s.notes">{{ s.notes }}</template>
                </div>
              </div>
            </div>
          </div>
        </div>

        <RouterLink class="btn btn--block" :to="`/fields/${fieldId}/rotation`">Open full rotation plan</RouterLink>
      </template>

      <!-- WATER -->
      <template v-else-if="tab === 'water'">
        <div class="card">
          <div class="card__head">Water - last 7 days ({{ water?.from }} to {{ water?.to }})</div>
          <div class="card__body">
            <div class="stat" :class="(water?.totalMm || 0) >= 25 ? 'stat--green' : 'stat--amber'">
              <div class="stat__label">Total</div>
              <div class="stat__value">{{ fmt(water?.totalMm || 0) }}<span class="stat__unit">mm</span></div>
            </div>
            <ul class="kv">
              <li><span>Rainfall</span><strong>{{ fmt(water?.rainfallMm || 0) }} mm</strong></li>
              <li><span>Irrigation</span><strong>{{ fmt(water?.irrigationMm || 0) }} mm</strong></li>
            </ul>
            <div v-if="water?.events.length" class="stack stack--tight" style="margin-top: 8px">
              <div v-for="e in water.events" :key="e.id" class="water-day">
                <span class="tiny">{{ e.date }}</span>
                <span class="small">
                  {{ fmt(e.rainfallMm) }} mm rain<template v-if="e.irrigationMm"> + {{ fmt(e.irrigationMm) }} mm</template>
                </span>
              </div>
            </div>
            <div class="tiny muted" style="margin-top: 8px">
              Rainfall is entered manually. Combined rainfall plus irrigation feeds the water card.
            </div>
            <div class="row" style="gap: 6px; margin-top: 8px">
              <button class="btn btn--sm" @click="addEntry('water')">Log water</button>
              <button class="btn btn--ghost btn--sm" @click="openBom">BOM rainfall</button>
            </div>
          </div>
        </div>
      </template>
    </template>

    <EntryDialog
      v-if="showEntry"
      :field-id="fieldId"
      :field-name="field?.name"
      :entry="editEntry?.id ? editEntry : null"
      :mixes="mixes"
      @cancel="showEntry = false; editEntry = null"
      @saved="afterSave"
      @changed-mixes="load"
    />
  </div>
</template>