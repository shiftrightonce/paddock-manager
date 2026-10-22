<script setup>
import { computed, onMounted, ref } from 'vue';
import { api, download, links } from '../api.js';
import { guard, loadFarms, store, toast } from '../composables/store.js';
import { fmt } from '../composables/format.js';

const fields = ref([]);
const loading = ref(true);
const report = ref(null);
const reportBusy = ref(false);
const scope = ref(null);

const allFields = computed(() => fields.value);

async function load() {
  loading.value = true;
  await guard(async () => {
    if (!store.farms.length) await loadFarms();
    const lists = await Promise.all(store.farms.map((f) => api.get(`/api/fields?farmId=${f.id}`)));
    fields.value = lists.flat();
  });
  loading.value = false;
}

function pick(kind, item) {
  scope.value = { kind, id: item.id, label: kind === 'farm' ? item.name : `${item.farm_name} / ${item.name}` };
  report.value = null;
}

async function preview() {
  if (!scope.value) return;
  reportBusy.value = true;
  await guard(async () => {
    report.value = await api.get(`/api/export/${scope.value.kind}/${scope.value.id}/preview`);
  });
  reportBusy.value = false;
}

function exportPdf() {
  if (!scope.value) return;
  download(`/api/export/${scope.value.kind}/${scope.value.id}.pdf`);
}

async function openBom() {
  await guard(async () => {
    window.open(await links.bom(store.farms.find((f) => f.id === store.currentFarmId)?.location), '_blank', 'noopener');
  });
}

async function openPubcris() {
  const q = window.prompt('Product name to search in PubCRIS');
  if (!q) return;
  await guard(async () => {
    window.open(await links.pubcris(q), '_blank', 'noopener');
  });
}

const today = new Date().toISOString().slice(0, 10);

onMounted(load);
</script>

<template>
  <div class="stack">
    <h2 class="section-title" style="margin: 0">Export summaries</h2>
    <div class="alert alert--info">
      <span class="alert__icon">i</span>
      <div>
        Reports are generated on demand. A field report covers status, water, nutrient budget,
        resistance, MOA history and the full entry timeline.
      </div>
    </div>

    <div class="grid-2">
      <div class="card">
        <div class="card__head">Farm reports</div>
        <div class="card__body card__body--flush">
          <div v-for="farm in store.farms" :key="farm.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe stripe-green" />
            <button class="field-row__main" type="button" @click="pick('farm', farm)">
              <div class="row row--between">
                <span class="field-row__name">{{ farm.name }}</span>
                <span class="tiny muted">{{ farm.fieldCount }} fields</span>
              </div>
              <div class="field-row__meta">
                {{ farm.redCount }} red · {{ farm.amberCount }} amber
              </div>
            </button>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card__head">Field reports</div>
        <div class="card__body card__body--flush">
          <div v-if="loading" class="skeleton" style="height: 140px; margin: 10px" />
          <div v-for="field in allFields" :key="field.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe" :class="`stripe-${field.status}`" />
            <button class="field-row__main" type="button" @click="pick('field', field)">
              <div class="row row--between">
                <span class="field-row__name">{{ field.name }}</span>
                <span class="chip" :class="`chip--${field.status}`">
                  {{ field.status === 'red' ? 'Red' : field.status === 'amber' ? 'Amber' : 'Green' }}
                </span>
              </div>
              <div class="field-row__meta">{{ field.farm_name }}</div>
            </button>
          </div>
        </div>
      </div>
    </div>

    <div v-if="scope" class="card">
      <div class="card__head">{{ scope.kind === 'farm' ? 'Farm' : 'Field' }} report: {{ scope.label }}</div>
      <div class="card__body">
        <div class="btn-group">
          <button class="btn btn--primary btn--sm" :disabled="reportBusy" @click="preview">
            {{ reportBusy ? 'Building...' : 'Preview' }}
          </button>
          <button class="btn btn--sm" @click="exportPdf">Download PDF</button>
        </div>

        <div v-if="report" class="stack" style="margin-top: 12px">
          <div class="stat-grid">
            <div class="stat">
              <div class="stat__label">As at</div>
              <div class="stat__value" style="font-size: 15px">{{ report.today || today }}</div>
            </div>
            <div class="stat stat--red">
              <div class="stat__label">Red fields</div>
              <div class="stat__value">{{ report.counts?.red ?? 0 }}</div>
            </div>
            <div class="stat stat--amber">
              <div class="stat__label">Amber fields</div>
              <div class="stat__value">{{ report.counts?.amber ?? 0 }}</div>
            </div>
          </div>

          <template v-if="report.fields">
            <div class="table-wrap">
              <table class="table">
                <thead>
                  <tr>
                    <th>Field</th>
                    <th>Status</th>
                    <th class="num">Area (ha)</th>
                    <th class="num">Resistance records</th>
                    <th>Flags</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="f in report.fields" :key="f.id">
                    <td>{{ f.name }}</td>
                    <td><span class="chip" :class="`chip--${f.status}`">{{ f.status }}</span></td>
                    <td class="num">{{ f.area_ha == null ? '-' : fmt(f.area_ha) }}</td>
                    <td class="num">{{ f.resistanceCount }}</td>
                    <td class="tiny">{{ f.statusText }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </template>

          <template v-else-if="report.field">
            <ul class="kv">
              <li><span>Status</span><strong>{{ report.field.status }}</strong></li>
              <li><span>Area</span><strong>{{ fmt(report.field.area_ha) }} ha</strong></li>
              <li><span>Rainfall 7d</span><strong>{{ fmt(report.water?.rainfallMm || 0) }} mm</strong></li>
              <li><span>Irrigation 7d</span><strong>{{ fmt(report.water?.irrigationMm || 0) }} mm</strong></li>
              <li><span>Resistances</span><strong>{{ (report.resistance || []).length }}</strong></li>
              <li><span>Entries</span><strong>{{ (report.entries || []).length }}</strong></li>
            </ul>
            <div class="table-wrap">
              <table class="table">
                <thead>
                  <tr>
                    <th>Date</th>
                    <th>Entry</th>
                    <th>Detail</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="e in report.entries" :key="e.id">
                    <td>{{ e.entry_date }}</td>
                    <td>{{ e.title }}</td>
                    <td class="tiny">{{ e.notes }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </template>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card__head">External lookups</div>
      <div class="card__body">
        <div class="btn-group">
          <button class="btn btn--sm" @click="openBom">BOM rainfall</button>
          <button class="btn btn--sm" @click="openPubcris">Search PubCRIS</button>
        </div>
        <div class="tiny muted">
          Both open in a new tab. No credentials or data are stored externally.
        </div>
      </div>
    </div>
  </div>
</template>