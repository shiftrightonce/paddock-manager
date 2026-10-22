/**
 * Formatting helpers used across the UI. All units are metric (mm, ha,
 * L/ha, kg/ha, g/L) per the spec.
 */

export function fmt(value, digits = 1) {
  if (value === null || value === undefined || value === '') return '-';
  const n = Number(value);
  if (!Number.isFinite(n)) return '-';
  return n.toLocaleString('en-AU', { maximumFractionDigits: digits });
}

export function fmtInt(value) {
  if (value === null || value === undefined || value === '') return '-';
  const n = Number(value);
  if (!Number.isFinite(n)) return '-';
  return n.toLocaleString('en-AU', { maximumFractionDigits: 0 });
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** ISO date to a compact, sortable-looking label: 12 Mar 2026 */
export function fmtDate(iso) {
  if (!iso) return '-';
  const d = new Date(`${String(iso).slice(0, 10)}T00:00:00`);
  if (Number.isNaN(d.getTime())) return String(iso);
  return `${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()}`;
}

export function fmtDateShort(iso) {
  if (!iso) return '-';
  const d = new Date(`${String(iso).slice(0, 10)}T00:00:00`);
  if (Number.isNaN(d.getTime())) return String(iso);
  return `${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

/** "3 days ago" / "in 5 days" / "today" */
export function relativeDays(days) {
  if (days === null || days === undefined) return '';
  if (days === 0) return 'today';
  if (days === 1) return 'tomorrow';
  if (days === -1) return 'yesterday';
  if (days > 0) return `in ${days} days`;
  return `${Math.abs(days)} days ago`;
}

export function todayIso() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export function addDaysIso(iso, days) {
  const d = new Date(`${iso}T00:00:00`);
  d.setDate(d.getDate() + days);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export const STATUS_LABEL = { red: 'Resistance', amber: 'Residual', green: 'Clear' };

export const ENTRY_TYPE_LABEL = {
  planting: 'Planting',
  soil_test: 'Soil test',
  soil_moisture: 'Soil moisture',
  weed_scouting: 'Weed scouting',
  chemical_application: 'Chemical application',
  harvest: 'Harvest',
  water: 'Rainfall / irrigation',
  inspection: 'Inspection',
};

export const TASK_TYPE_LABEL = {
  insect_scouting: 'Insect scouting',
  crop_inspection: 'Crop inspection',
  custom: 'Custom',
};

export const REPEAT_LABEL = {
  none: 'Does not repeat',
  weekly: 'Weekly',
  fortnightly: 'Fortnightly',
  monthly: 'Monthly',
};

export const IRRIGATION_LABEL = {
  pivot: 'Pivot',
  flood: 'Flood',
  drip: 'Drip',
  spray: 'Spray',
  furrow: 'Furrow',
  none: 'None',
};

export function titleCase(s) {
  return String(s || '')
    .replace(/[_-]+/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}