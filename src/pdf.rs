//! PDF rendering for farm and field summaries.
//!
//! A4 with 40pt margins and the built-in Helvetica faces, so no font has to be
//! embedded. Layout is done by hand in points because the reports are simple
//! enough that a flow-layout engine would only hide where the page breaks fall.
//! Content that would land too near the bottom of a page starts a new one, and
//! footers are stamped once the page count is known.

use printpdf::*;

use crate::http::dashboard::Report;

const PAGE_W: f32 = 595.28; // A4 in points
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 40.0;
const CONTENT_WIDTH: f32 = PAGE_W - MARGIN * 2.0;

/// Colours are 0.0-1.0 components, not bytes.
const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        icc_profile: None,
    }
}

const INK: Rgb = rgb(31, 41, 55);
const MUTED: Rgb = rgb(107, 114, 128);
const RULE: Rgb = rgb(209, 213, 219);
const RED: Rgb = rgb(185, 28, 28);
const AMBER: Rgb = rgb(180, 83, 9);
const GREEN: Rgb = rgb(21, 128, 61);

fn colour(c: Rgb) -> Color {
    Color::Rgb(c)
}

fn pt(x: f32, y: f32) -> Point {
    Point { x: Pt(x), y: Pt(y) }
}

fn line_point(x: f32, y: f32) -> LinePoint {
    LinePoint {
        p: pt(x, y),
        bezier: false,
    }
}

/// One text run, positioned absolutely.
fn text_ops(text: &str, x: f32, y: f32, size: f32, c: Rgb, font: BuiltinFont) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetTextCursor { pos: pt(x, y) },
        Op::SetFontSizeBuiltinFont {
            size: Pt(size),
            font,
        },
        Op::SetLineHeight { lh: Pt(size * 1.2) },
        Op::SetFillColor { col: colour(c) },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(text.to_string())],
            font,
        },
        Op::EndTextSection,
    ]
}

fn line_ops(x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, c: Rgb) -> Vec<Op> {
    vec![
        Op::SetOutlineColor { col: colour(c) },
        Op::SetOutlineThickness { pt: Pt(thickness) },
        Op::DrawLine {
            line: Line {
                points: vec![line_point(x1, y1), line_point(x2, y2)],
                is_closed: false,
            },
        },
    ]
}

fn rect_ops(x: f32, y: f32, w: f32, h: f32, c: Rgb) -> Vec<Op> {
    vec![
        Op::SetFillColor { col: colour(c) },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        line_point(x, y),
                        line_point(x + w, y),
                        line_point(x + w, y + h),
                        line_point(x, y + h),
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
    ]
}

/// A filled dot, approximated with a polygon because printpdf has no circle op.
fn dot_ops(cx: f32, cy: f32, radius: f32, c: Rgb) -> Vec<Op> {
    let segments = 12;
    let points = (0..segments)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / segments as f32;
            line_point(cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect();
    vec![
        Op::SetFillColor { col: colour(c) },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing { points }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
    ]
}

/// Cursor over the pages, opening a new one as content runs out.
struct Doc {
    pages: Vec<Vec<Op>>,
    y: f32,
}

impl Doc {
    fn new() -> Self {
        Self {
            pages: vec![Vec::new()],
            y: PAGE_H - MARGIN,
        }
    }

    /// Reserve `height` points, breaking to a new page when it will not fit.
    fn need(&mut self, height: f32) {
        while self.y - height < MARGIN + 24.0 {
            self.pages.push(Vec::new());
            self.y = PAGE_H - MARGIN;
        }
    }

    fn push(&mut self, ops: Vec<Op>) {
        self.pages
            .last_mut()
            .expect("a page is always open")
            .extend(ops);
    }

    fn blank(&mut self, h: f32) {
        self.y -= h;
    }

    fn rule(&mut self) {
        self.need(14.0);
        self.blank(8.0);
        self.push(line_ops(
            MARGIN,
            self.y,
            MARGIN + CONTENT_WIDTH,
            self.y,
            0.8,
            RULE,
        ));
        self.blank(6.0);
    }

    fn heading(&mut self, text: &str) {
        self.need(40.0);
        self.blank(18.0);
        self.push(text_ops(
            text,
            MARGIN,
            self.y,
            14.0,
            INK,
            BuiltinFont::HelveticaBold,
        ));
        self.blank(4.0);
        self.rule();
    }

    fn title(&mut self, text: &str, subtitle: &str) {
        self.blank(10.0);
        self.push(text_ops(
            text,
            MARGIN,
            self.y,
            20.0,
            INK,
            BuiltinFont::HelveticaBold,
        ));
        self.blank(16.0);
        if !subtitle.is_empty() {
            self.push(text_ops(
                subtitle,
                MARGIN,
                self.y,
                11.0,
                MUTED,
                BuiltinFont::Helvetica,
            ));
            self.blank(8.0);
        }
    }

    /// A label/value pair, with the value wrapped under the label when long.
    fn field(&mut self, label: &str, value: &str) {
        self.need(16.0);
        self.push(text_ops(
            &format!("{label}: "),
            MARGIN,
            self.y,
            10.0,
            MUTED,
            BuiltinFont::Helvetica,
        ));
        let label_width = text_width(&format!("{label}: "), 10.0);
        let available = CONTENT_WIDTH - label_width;

        if text_width(value, 10.0) <= available {
            self.push(text_ops(
                value,
                MARGIN + label_width,
                self.y,
                10.0,
                INK,
                BuiltinFont::Helvetica,
            ));
            self.blank(14.0);
        } else {
            self.blank(14.0);
            for line in wrap(value, available, 10.0) {
                self.need(14.0);
                self.push(text_ops(
                    &line,
                    MARGIN + label_width,
                    self.y,
                    10.0,
                    INK,
                    BuiltinFont::Helvetica,
                ));
                self.blank(12.0);
            }
        }
    }

    fn bullet(&mut self, text: &str, c: &Rgb) {
        for (i, line) in wrap(text, CONTENT_WIDTH - 14.0, 9.5)
            .into_iter()
            .enumerate()
        {
            self.need(13.0);
            let indent = if i == 0 { 0.0 } else { 10.0 };
            if i == 0 {
                self.push(dot_ops(MARGIN + 3.0, self.y + 3.0, 1.6, c.clone()));
            }
            self.push(text_ops(
                &line,
                MARGIN + 10.0 + indent,
                self.y,
                9.5,
                c.clone(),
                BuiltinFont::Helvetica,
            ));
            self.blank(12.0);
        }
    }

    /// A colour swatch followed by a field name and detail.
    fn status_row(&mut self, status: &str, name: &str, detail: &str) {
        self.need(30.0);
        let c = status_colour(status);
        self.push(rect_ops(MARGIN, self.y - 2.0, 4.0, 14.0, c));
        self.push(text_ops(
            name,
            MARGIN + 12.0,
            self.y,
            11.0,
            INK,
            BuiltinFont::HelveticaBold,
        ));
        self.blank(13.0);
        if !detail.is_empty() {
            for line in wrap(detail, CONTENT_WIDTH - 12.0, 9.0) {
                self.need(12.0);
                self.push(text_ops(
                    &line,
                    MARGIN + 12.0,
                    self.y,
                    9.0,
                    MUTED,
                    BuiltinFont::Helvetica,
                ));
                self.blank(11.0);
            }
        }
        self.blank(4.0);
    }

    /// One timeline entry: date, title, then its detail lines.
    fn timeline(&mut self, date: &str, title: &str, lines: &[String], conflicted: bool) {
        self.need(26.0);
        let c = if conflicted { RED } else { INK };
        self.push(text_ops(
            date,
            MARGIN,
            self.y,
            9.0,
            MUTED,
            BuiltinFont::Helvetica,
        ));
        self.push(text_ops(
            title,
            MARGIN + 76.0,
            self.y,
            10.5,
            c,
            BuiltinFont::HelveticaBold,
        ));
        self.blank(13.0);

        for line in lines {
            self.bullet(line, &if conflicted { RED } else { INK });
        }
        self.blank(3.0);
    }

    /// Stamp the footer on every page, then hand the pages to printpdf.
    fn finish(mut self, title: &str) -> Vec<u8> {
        let total = self.pages.len();
        for (index, page) in self.pages.iter_mut().enumerate() {
            page.extend(text_ops(
                &format!("Paddock Manager  |  page {} of {total}", index + 1),
                MARGIN,
                MARGIN - 14.0,
                8.0,
                MUTED,
                BuiltinFont::Helvetica,
            ));
        }

        let pages = self
            .pages
            .into_iter()
            .map(|ops| PdfPage::new(Mm(210.0), Mm(297.0), ops))
            .collect();

        PdfDocument::new(title)
            .with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut Vec::new())
    }
}

fn status_colour(level: &str) -> Rgb {
    match level {
        "red" => RED,
        "amber" => AMBER,
        _ => GREEN,
    }
}

/// Helvetica advance widths are close enough to 0.5em for wrapping purposes.
/// Helvetica advance widths are close enough to 0.5em for wrapping purposes.
fn text_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.5
}

fn wrap(text: &str, max_width: f32, size: f32) -> Vec<String> {
    if text_width(text, size) <= max_width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if text_width(&candidate, size) <= max_width {
            current = candidate;
        } else {
            if !current.is_empty() {
                lines.push(current);
            }
            // A single word longer than the line is broken by character.
            if text_width(word, size) > max_width {
                let mut chunk = String::new();
                for c in word.chars() {
                    if text_width(&format!("{chunk}{c}"), size) > max_width {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(c);
                }
                current = chunk;
            } else {
                current = word.to_string();
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Render a report to PDF bytes.
pub fn render(report: &Report) -> Result<Vec<u8>, anyhow::Error> {
    let mut d = Doc::new();

    d.title(&report.title, &report.subtitle);
    d.push(text_ops(
        &format!("Generated {}", report.generated_at),
        MARGIN,
        d.y,
        8.5,
        MUTED,
        BuiltinFont::Helvetica,
    ));
    d.blank(4.0);

    for (label, value) in &report.facts {
        d.field(label, value);
    }

    if !report.attention.is_empty() {
        d.heading("Needs attention");
        for item in &report.attention {
            let level = item["level"].as_str().unwrap_or("green");
            let place = match (item["fieldName"].as_str(), item["farmName"].as_str()) {
                (Some(f), _) if !f.is_empty() => format!("{f} - "),
                (None, Some(fm)) if !fm.is_empty() => format!("{fm} - "),
                _ => String::new(),
            };
            let text = format!(
                "{place}{} [{}]",
                item["text"].as_str().unwrap_or_default(),
                item["date"].as_str().unwrap_or_default()
            );
            d.bullet(&text, &status_colour(level));
        }
    }

    if !report.upcoming.is_empty() {
        d.heading("Upcoming tasks");
        for t in &report.upcoming {
            let due = t["due_date"].as_str().unwrap_or_default();
            let marker = if t["overdue"].as_bool().unwrap_or(false) {
                "OVERDUE"
            } else if t["dueToday"].as_bool().unwrap_or(false) {
                "due today"
            } else {
                ""
            };
            let scope = match (t["field_name"].as_str(), t["farm_name"].as_str()) {
                (Some(f), _) if !f.is_empty() => f.to_string(),
                (None, Some(fm)) if !fm.is_empty() => fm.to_string(),
                _ => "Farm task".to_string(),
            };
            let repeat = t["repeat_interval"].as_str().unwrap_or("none");
            let repeat_text = if repeat != "none" {
                format!(" ({repeat})")
            } else {
                String::new()
            };
            let text = format!(
                "{due} - {} - {scope}{repeat_text} {marker}",
                t["title"].as_str().unwrap_or_default()
            );
            let c = if t["overdue"].as_bool().unwrap_or(false) {
                RED
            } else {
                INK
            };
            d.bullet(text.trim(), &c);
        }
    }

    if !report.fields.is_empty() {
        d.heading("Fields");
        for (status, name, detail) in &report.fields {
            d.status_row(status, name, detail);
        }
    }

    if let Some(water) = &report.water {
        d.heading("Water input (last 7 days)");
        d.field(
            "Period",
            &format!(
                "{} to {}",
                water["from"].as_str().unwrap_or_default(),
                water["to"].as_str().unwrap_or_default()
            ),
        );
        d.field(
            "Total",
            &format!("{} mm", water["totalMm"].as_f64().unwrap_or(0.0)),
        );
        d.field(
            "Rainfall",
            &format!("{} mm", water["rainfallMm"].as_f64().unwrap_or(0.0)),
        );
        d.field(
            "Irrigation",
            &format!("{} mm", water["irrigationMm"].as_f64().unwrap_or(0.0)),
        );
    }

    if let Some(residual) = &report.residual {
        if !residual.is_null() {
            d.heading("Residual status");
            let names = residual["residuals"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            d.field(
                "Last applied",
                residual["date"].as_str().unwrap_or_default(),
            );
            d.field("Products", &names);
            d.field(
                "Days since",
                &residual["daysSince"]
                    .as_i64()
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            );
            d.bullet(
                "Plantback is not calculated here. Check the interval on the product label.",
                &AMBER,
            );
        }
    }

    if !report.resistance.is_empty() {
        d.heading("Weed resistance records");
        for r in &report.resistance {
            let species = r["weed_species"].as_str().unwrap_or_default();
            let mut detail = Vec::new();
            if let Some(g) = r["moa_group"].as_str().filter(|s| !s.is_empty()) {
                detail.push(format!("Group {g}"));
            }
            if let Some(a) = r["active"].as_str().filter(|s| !s.is_empty()) {
                detail.push(a.to_string());
            }
            if let Some(p) = r["product"].as_str().filter(|s| !s.is_empty()) {
                detail.push(p.to_string());
            }
            d.bullet(&format!("{species} - {}", detail.join(" / ")), &RED);
        }
    }

    if !report.timeline.is_empty() {
        d.heading("Entry timeline");
        for (date, title, lines, conflicted) in &report.timeline {
            d.timeline(date, title, lines, *conflicted);
        }
    }

    Ok(d.finish(&format!("Paddock Manager - {}", report.title)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_does_not_wrap() {
        assert_eq!(wrap("Roundup", 500.0, 10.0).len(), 1);
    }

    #[test]
    fn long_text_wraps_on_word_boundaries() {
        let lines = wrap("the quick brown fox jumps over the lazy dog", 100.0, 10.0);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| text_width(l, 10.0) <= 100.0));
        assert!(lines.join(" ").contains("quick"));
    }

    #[test]
    fn an_oversized_word_is_broken_by_character() {
        let long = "A".repeat(120);
        let lines = wrap(&long, 60.0, 10.0);
        assert!(lines.len() > 1);
        assert_eq!(lines.concat(), long, "no characters are dropped");
    }
}
