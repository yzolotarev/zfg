use std::{
    cell::{Cell, RefCell},
    f64::consts::PI,
    rc::Rc,
};

#[derive(Default)]
struct Hover {
    target: Cell<Option<usize>>,
    levels: Vec<Cell<f64>>,
    generation: Cell<u64>,
    tick: RefCell<Option<gtk::TickCallbackId>>,
}

fn hover_level(from: f64, to: f64, elapsed_us: i64) -> f64 {
    let progress = (elapsed_us as f64 / 100_000.0).clamp(0.0, 1.0);
    from + (to - from) * (1.0 - (1.0 - progress).powi(3))
}

impl Hover {
    fn transition(self: &Rc<Self>, canvas: &DrawingArea, target: Option<usize>, animate: bool) {
        if self.target.replace(target) == target && animate {
            return;
        }
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        if let Some(tick) = self.tick.borrow_mut().take() {
            tick.remove();
        }
        if !animate {
            for (i, level) in self.levels.iter().enumerate() {
                level.set(if target == Some(i) { 1.0 } else { 0.0 });
            }
            canvas.queue_draw();
            return;
        }
        let from: Vec<f64> = self.levels.iter().map(Cell::get).collect();
        let start = canvas.frame_clock().map(|clock| clock.frame_time());
        let weak = Rc::downgrade(self);
        let start = Cell::new(start);
        let tick = canvas.add_tick_callback(move |canvas, clock| {
            let Some(state) = weak.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            if state.generation.get() != generation {
                return gtk::glib::ControlFlow::Break;
            }
            let started = start.get().unwrap_or_else(|| {
                start.set(Some(clock.frame_time()));
                clock.frame_time()
            });
            let elapsed = clock.frame_time() - started;
            let animate = animations_enabled(canvas);
            for (i, level) in state.levels.iter().enumerate() {
                let to = if target == Some(i) { 1.0 } else { 0.0 };
                level.set(hover_level(
                    from[i],
                    to,
                    if animate { elapsed } else { 100_000 },
                ));
            }
            canvas.queue_draw();
            if !animate || elapsed >= 100_000 {
                state.tick.borrow_mut().take();
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
        self.tick.replace(Some(tick));
    }
}

fn animations_enabled(canvas: &DrawingArea) -> bool {
    canvas.settings().is_gtk_enable_animations()
}

use gtk::{
    Align, Box as GtkBox, Button, CssProvider, DrawingArea, EventControllerMotion, Fixed, Image,
    Label, Orientation, Overlay, pango, prelude::*,
};

pub(crate) struct Pie {
    pub root: Overlay,
    pub segments: Vec<Button>,
    pub cancel: Button,
}

fn wedge_at(x: f64, y: f64, width: f64, radius: f64, count: usize) -> Option<usize> {
    let x = x - width / 2.0;
    let y = y - 22.0;
    let distance = x.hypot(y);
    if count == 0 || distance < radius * 0.30 || distance > radius + 1e-9 || y < 0.0 {
        return None;
    }
    let angle = y.atan2(x);
    let index = ((PI - angle) / PI * count as f64).floor() as usize;
    let index = index.min(count - 1);
    let start = PI * (1.0 - index as f64 / count as f64) - 0.025;
    let end = PI * (1.0 - (index + 1) as f64 / count as f64) + 0.025;
    (angle >= end && angle <= start).then_some(index)
}

fn presentation(label: &str) -> (&str, String) {
    let label = label.trim();
    let (icon, text) = match label.split_once(' ') {
        Some(("💡", text)) => ("document-new-symbolic", text),
        Some(("▦", text)) => ("x-office-calendar-symbolic", text),
        Some(("✓", text)) => ("object-select-symbolic", text),
        Some(("◉", text)) => ("view-reveal-symbolic", text),
        Some(("≡", text)) => ("view-list-symbolic", text),
        _ => ("application-x-executable-symbolic", label),
    };
    // Shortcut tags are presentation metadata; keep any following capacity badge.
    let mut text = text.to_owned();
    if let Some(start) = text.rfind(" [")
        && let Some(end) = text[start..].find(']')
    {
        text.replace_range(start..start + end + 1, "");
    }
    (icon, text.trim().to_owned())
}

// Find a rectangular content box whose corners stay inside the annular wedge.
fn content_width(
    outer: f64,
    inner: f64,
    angle: f64,
    half_span: f64,
    distance: f64,
    height: f64,
) -> i32 {
    let x = distance * angle.cos();
    let y = distance * angle.sin();
    let fits = |width: f64| {
        [-1.0, 1.0].into_iter().all(|sx| {
            [-1.0, 1.0].into_iter().all(|sy| {
                let px = x + sx * width / 2.0;
                let py = y + sy * height / 2.0;
                let r = px.hypot(py);
                r >= inner + 3.0 && r <= outer - 4.0 && (py.atan2(px) - angle).abs() <= half_span
            })
        })
    };
    let mut low = 0.0;
    let mut high = outer;
    for _ in 0..16 {
        let middle = (low + high) / 2.0;
        if fits(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    low.floor() as i32
}

pub(crate) fn build(
    labels: &[String],
    warning: Option<usize>,
    radius: f64,
    show_cancel: bool,
) -> Pie {
    let segment_count = labels.len();
    let radius = if segment_count == 2 { 105.0 } else { radius };
    let width = (radius * 2.0 + 24.0).ceil() as i32;
    let top = 22.0;
    let height = (radius + top + 8.0).ceil() as i32;
    let inner = radius * 0.30;
    let gap = 0.025;
    let hovered = Rc::new(Hover {
        levels: (0..segment_count).map(|_| Cell::new(0.0)).collect(),
        ..Hover::default()
    });
    let focused = Rc::new(Cell::new(None::<usize>));
    let selected: Rc<Vec<Cell<bool>>> =
        Rc::new((0..segment_count).map(|_| Cell::new(false)).collect());
    let drawing = DrawingArea::new();
    drawing.set_content_width(width);
    drawing.set_content_height(height);
    let draw_hovered = hovered.clone();
    let draw_focused = focused.clone();
    let draw_selected = selected.clone();
    drawing.set_draw_func(move |area, context, width, _| {
        let cx = width as f64 / 2.0;
        let foreground = area.style_context().color();
        for i in 0..segment_count {
            let start = PI * (1.0 - i as f64 / segment_count as f64) - gap;
            let end = PI * (1.0 - (i + 1) as f64 / segment_count as f64) + gap;
            context.new_path();
            let hover = draw_hovered.levels[i].get();
            context.arc_negative(cx, top, radius + 3.0 * hover, start, end);
            context.arc(cx, top, inner, end, start);
            context.close_path();

            let focus = draw_focused.get() == Some(i) || draw_selected[i].get();
            if warning == Some(i) {
                let lift = 0.07 * hover;
                context.set_source_rgba(0.30 + lift, 0.16 + lift, 0.17 + lift, 0.96);
            } else {
                let neutral = 0.14 + 0.09 * hover;
                context.set_source_rgba(neutral, neutral, neutral, 0.96);
            }
            let _ = context.fill_preserve();
            context.set_source_rgba(
                foreground.red() as f64,
                foreground.green() as f64,
                foreground.blue() as f64,
                if focus { 0.95 } else { 0.20 + 0.25 * hover },
            );
            context.set_line_width(if focus { 2.0 } else { 1.0 });
            let _ = context.stroke();
        }
    });
    let root = Overlay::new();
    root.set_halign(Align::Center);
    root.set_child(Some(&drawing));
    let fixed = Fixed::new();
    root.add_overlay(&fixed);

    // Local fallback rules remove theme button backing; the parent can refine this class.
    let css = CssProvider::new();
    css.load_from_data(
        ".zfg-pie-segment, .zfg-pie-segment:hover, .zfg-pie-segment:active, \
         .zfg-pie-segment:checked { background: transparent; background-image: none; \
         box-shadow: none; border: none; border-radius: 0; padding: 0; min-width: 0; \
         min-height: 0; outline: none; } \
         .zfg-pie-segment label { font-size: 0.85em; color: #f4f5f7; } \
         .zfg-pie-cancel { border-radius: 999px; padding: 0; min-width: 0; \
         min-height: 0; }",
    );
    let cx = width as f64 / 2.0;
    let mut segments = Vec::with_capacity(segment_count);
    for (i, label) in labels.iter().enumerate() {
        let angle = PI * (1.0 - (i as f64 + 0.5) / segment_count as f64);
        let distance = radius * if segment_count == 2 { 0.64 } else { 0.66 };
        let button_height = if radius < 100.0 {
            28
        } else if segment_count == 2 {
            40
        } else {
            46
        };
        let button_width = content_width(
            radius,
            inner,
            angle,
            PI / (2.0 * segment_count as f64) - gap - 0.02,
            distance,
            button_height as f64,
        )
        .max(1);
        let (icon_name, text) = presentation(label);
        let icon = Image::from_icon_name(icon_name);
        icon.set_pixel_size(if radius < 100.0 { 12 } else { 18 });
        let caption = Label::new(Some(&text));
        caption.set_ellipsize(pango::EllipsizeMode::End);
        caption.set_max_width_chars((button_width / 8).max(4));
        caption.set_hexpand(true);
        let content = GtkBox::new(Orientation::Vertical, 2);
        content.set_halign(Align::Fill);
        content.set_valign(Align::Center);
        content.append(&icon);
        content.append(&caption);
        let button = Button::new();
        button.set_child(Some(&content));
        button.set_tooltip_text(Some(label));
        button.add_css_class("flat");
        button.add_css_class("zfg-pie-segment");
        button
            .style_context()
            .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        if warning == Some(i) {
            button.add_css_class("warning");
        }
        button.set_size_request(button_width, button_height);

        let state = selected.clone();
        let canvas = drawing.clone();
        button.connect_css_classes_notify(move |button| {
            state[i].set(button.has_css_class("selected"));
            canvas.queue_draw();
        });

        let state = focused.clone();
        let canvas = drawing.clone();
        button.connect_has_focus_notify(move |button| {
            if button.has_focus() {
                state.set(Some(i));
            } else if state.get() == Some(i) {
                state.set(None);
            }
            canvas.queue_draw();
        });
        fixed.put(
            &button,
            cx + distance * angle.cos() - button_width as f64 / 2.0,
            top + distance * angle.sin() - button_height as f64 / 2.0,
        );
        segments.push(button);
    }
    // Capture the real annular geometry, rather than enlarging rectangular buttons
    // into neighbouring wedges. Keep the buttons for keyboard/accessibility activation.
    let motion = EventControllerMotion::new();
    motion.set_propagation_phase(gtk::PropagationPhase::Capture);
    let state = hovered.clone();
    let canvas = drawing.clone();
    motion.connect_motion(move |controller, x, y| {
        if let Some(widget) = controller.widget() {
            state.transition(
                &canvas,
                wedge_at(x, y, widget.width() as f64, radius, segment_count),
                animations_enabled(&canvas),
            );
        }
    });
    let state = hovered.clone();
    let canvas = drawing.clone();
    motion.connect_leave(move |_| {
        state.transition(&canvas, None, animations_enabled(&canvas));
    });
    root.add_controller(motion);
    {
        let settings = drawing.settings();
        let state = Rc::downgrade(&hovered);
        let canvas = drawing.downgrade();
        settings.connect_gtk_enable_animations_notify(move |settings| {
            if !settings.is_gtk_enable_animations()
                && let (Some(state), Some(canvas)) = (state.upgrade(), canvas.upgrade())
            {
                state.transition(&canvas, state.target.get(), false);
            }
        });
    }
    let click = gtk::GestureClick::new();
    click.set_button(1);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed = Rc::new(Cell::new(None));
    let state = pressed.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        let hit = gesture
            .widget()
            .and_then(|widget| wedge_at(x, y, widget.width() as f64, radius, segment_count));
        state.set(hit);
        if hit.is_some() {
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    let targets = segments.clone();
    click.connect_released(move |gesture, _, x, y| {
        let hit = gesture
            .widget()
            .and_then(|widget| wedge_at(x, y, widget.width() as f64, radius, segment_count));
        if let Some(index) = pressed.take().filter(|index| Some(*index) == hit)
            && targets[index].is_sensitive()
        {
            targets[index].grab_focus();
            targets[index].emit_clicked();
        }
    });
    root.add_controller(click);
    let cancel = Button::from_icon_name("window-close-symbolic");
    cancel.add_css_class("flat");
    cancel.add_css_class("zfg-pie-cancel");
    cancel
        .style_context()
        .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    cancel.set_size_request(28, 28);
    fixed.put(&cancel, cx - 14.0, top - 14.0);
    cancel.set_visible(show_cancel);
    Pie {
        root,
        segments,
        cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_is_cubic_out_bounded_and_finishes_at_100ms() {
        assert_eq!(hover_level(0.0, 1.0, 0), 0.0);
        assert_eq!(hover_level(0.0, 1.0, 50_000), 0.875);
        assert_eq!(hover_level(0.0, 1.0, 100_000), 1.0);
        assert_eq!(hover_level(0.0, 1.0, 200_000), 1.0);
        assert_eq!(hover_level(0.875, 0.0, 0), 0.875);
        assert_eq!(hover_level(0.875, 0.0, 100_000), 0.0);
        for elapsed in (0..=100_000).step_by(1000) {
            assert!((0.0..=3.0).contains(&(3.0 * hover_level(0.0, 1.0, elapsed))));
        }
    }

    #[test]
    fn wedge_targets_cover_the_annulus_and_exclude_gaps() {
        for count in [2, 3] {
            let radius = 142.0;
            let width = radius * 2.0 + 24.0;
            for index in 0..count {
                let angle = PI * (1.0 - (index as f64 + 0.5) / count as f64);
                for distance in [radius * 0.31, radius * 0.9, radius] {
                    assert_eq!(
                        wedge_at(
                            width / 2.0 + distance * angle.cos(),
                            22.0 + distance * angle.sin(),
                            width,
                            radius,
                            count
                        ),
                        Some(index)
                    );
                }
            }
            assert_eq!(wedge_at(width / 2.0, 22.0, width, radius, count), None);
            assert_eq!(wedge_at(width / 2.0, 0.0, width, radius, count), None);
            let boundary = PI / count as f64;
            assert_eq!(
                wedge_at(
                    width / 2.0 + radius * 0.8 * boundary.cos(),
                    22.0 + radius * 0.8 * boundary.sin(),
                    width,
                    radius,
                    count
                ),
                None
            );
        }
    }

    #[test]
    fn labels_keep_localized_text_and_capacity_without_shortcuts() {
        assert_eq!(presentation("💡 Заметка [1]").1, "Заметка");
        assert_eq!(presentation("◉ Focus [F] 3/3").1, "Focus 3/3");
        assert_eq!(presentation("≡ Отложка [L]").1, "Отложка");
        assert_eq!(presentation("Plain text").1, "Plain text");
    }

    #[test]
    fn standard_wedges_have_room_for_content() {
        for (radius, count) in [(142.0, 3), (105.0, 2)] {
            for i in 0..count {
                let angle = PI * (1.0 - (i as f64 + 0.5) / count as f64);
                assert!(
                    content_width(
                        radius,
                        radius * 0.30,
                        angle,
                        PI / (2.0 * count as f64) - 0.045,
                        radius * if count == 2 { 0.64 } else { 0.66 },
                        if count == 2 { 40.0 } else { 46.0 },
                    ) >= 48
                );
            }
        }
    }
}
