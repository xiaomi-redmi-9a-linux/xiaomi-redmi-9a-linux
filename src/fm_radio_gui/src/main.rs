mod presets;
mod protocol;

use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, FlowBox,
    Label, Orientation, Scale, ScrolledWindow, StyleContext, WindowPosition,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const CSS: &str = r#"
window {
    background-color: #0d0f12;
    color: #e0e6ed;
    font-family: -apple-system, system-ui, sans-serif;
}

button {
    background-image: none;
    box-shadow: none;
    text-shadow: none;
}

.card {
    background-color: #181c22;
    border-radius: 18px;
    padding: 16px;
    margin: 8px 12px;
    border: 1px solid #28303d;
}

.header-title {
    font-size: 20px;
    font-weight: 700;
    color: #ffffff;
}

.freq-display {
    font-size: 64px;
    font-weight: 900;
    color: #00e5ff;
    letter-spacing: -1px;
}

.mhz-label {
    font-size: 22px;
    font-weight: 700;
    color: #8da0b8;
}

.rssi-label {
    font-size: 13px;
    font-weight: 600;
    color: #00e676;
    background-color: rgba(0, 230, 118, 0.16);
    border-radius: 12px;
    padding: 5px 12px;
}

.rssi-weak {
    color: #ffab00;
    background-color: rgba(255, 171, 0, 0.16);
    border-radius: 12px;
    padding: 5px 12px;
}

.rssi-off {
    color: #8da0b8;
    background-color: rgba(141, 160, 184, 0.12);
    border-radius: 12px;
    padding: 5px 12px;
}

.power-btn-on {
    background-color: #00e676;
    color: #000000;
    font-size: 18px;
    font-weight: 800;
    border-radius: 28px;
    min-height: 56px;
    border: 2px solid #00c853;
}

.power-btn-off {
    background-color: #242c38;
    color: #ffffff;
    font-size: 18px;
    font-weight: 800;
    border-radius: 28px;
    min-height: 56px;
    border: 2px solid #374354;
}

.tune-btn {
    background-color: #242c38;
    color: #ffffff;
    font-size: 16px;
    font-weight: 700;
    border-radius: 14px;
    min-height: 50px;
    min-width: 68px;
    border: 1px solid #374354;
}

.tune-btn:active {
    background-color: #00e5ff;
    color: #000000;
}

.preset-btn {
    background-color: #242c38;
    color: #ffffff;
    font-size: 15px;
    font-weight: 700;
    border-radius: 14px;
    padding: 8px 16px;
    min-height: 42px;
    border: 1px solid #374354;
}

.preset-btn-active {
    background-color: #00e5ff;
    color: #000000;
    font-weight: 800;
    border-radius: 14px;
    padding: 8px 16px;
    min-height: 42px;
    border: 1px solid #00e5ff;
}

.route-btn {
    background-color: #242c38;
    color: #b0bec5;
    font-size: 13px;
    font-weight: 700;
    border-radius: 12px;
    padding: 6px 14px;
    border: 1px solid #374354;
}

.route-btn-active {
    background-color: #00e5ff;
    color: #000000;
    font-size: 13px;
    font-weight: 700;
    border-radius: 12px;
    padding: 6px 14px;
    border: 1px solid #00e5ff;
}

.close-btn {
    background-color: #242c38;
    color: #ff5252;
    font-size: 16px;
    font-weight: 900;
    border-radius: 12px;
    padding: 6px 14px;
    border: 1px solid #ff5252;
}

.close-btn:active {
    background-color: #ff5252;
    color: #ffffff;
}

scale trough {
    background-color: #242c38;
    border-radius: 8px;
    min-height: 10px;
}

scale highlight {
    background-color: #00e5ff;
    border-radius: 8px;
    min-height: 10px;
}

scale slider {
    background-color: #ffffff;
    border-radius: 50%;
    min-width: 26px;
    min-height: 26px;
    border: 3px solid #00e5ff;
}
"#;

struct AppState {
    current_freq: f32,
    is_playing: bool,
    current_rssi: i32,
    current_route: String,
    current_vol: i32,
    presets: Vec<f32>,
    is_updating_ui: bool,
    is_scanning: bool,
}

struct AppWidgets {
    lbl_status_badge: Label,
    lbl_freq: Label,
    lbl_rssi: Label,
    slider_freq: Scale,
    btn_seek_down: Button,
    btn_step_down: Button,
    btn_step_up: Button,
    btn_seek_up: Button,
    btn_power: Button,
    slider_vol: Scale,
    lbl_vol: Label,
    btn_route_hp: Button,
    btn_route_spk: Button,
    flow_presets: FlowBox,
    btn_scan: Button,
}

struct AppContext {
    state: RefCell<AppState>,
    widgets: AppWidgets,
}

impl AppContext {
    fn sync_ui(&self) {
        let (freq, is_playing, rssi, route, vol, is_scanning) = {
            let mut st = self.state.borrow_mut();
            st.is_updating_ui = true;
            (
                st.current_freq,
                st.is_playing,
                st.current_rssi,
                st.current_route.clone(),
                st.current_vol,
                st.is_scanning,
            )
        };

        // Frequency Label & Slider
        self.widgets.lbl_freq.set_text(&format!("{:.1}", freq));
        self.widgets.slider_freq.set_value(freq as f64);

        // Power Button
        let ctx_pwr = self.widgets.btn_power.style_context();
        if is_playing {
            self.widgets.btn_power.set_label("⏹  POWER OFF");
            ctx_pwr.remove_class("power-btn-off");
            ctx_pwr.add_class("power-btn-on");
        } else {
            self.widgets.btn_power.set_label("⚡  POWER ON");
            ctx_pwr.remove_class("power-btn-on");
            ctx_pwr.add_class("power-btn-off");
        }

        // Status Badge & RSSI
        let ctx_badge = self.widgets.lbl_status_badge.style_context();
        let ctx_rssi = self.widgets.lbl_rssi.style_context();
        ctx_badge.remove_class("rssi-label");
        ctx_badge.remove_class("rssi-weak");
        ctx_badge.remove_class("rssi-off");
        ctx_rssi.remove_class("rssi-label");
        ctx_rssi.remove_class("rssi-weak");
        ctx_rssi.remove_class("rssi-off");

        if is_scanning {
            self.widgets.lbl_status_badge.set_text("SCANNING...");
            ctx_badge.add_class("rssi-weak");
            self.widgets.lbl_rssi.set_text("-- dBm");
            ctx_rssi.add_class("rssi-off");
        } else if is_playing {
            self.widgets.lbl_status_badge.set_text("● LIVE");
            ctx_badge.add_class("rssi-label");

            if rssi > -60 {
                self.widgets.lbl_rssi.set_text(&format!("{} dBm (EXCELLENT)", rssi));
                ctx_rssi.add_class("rssi-label");
            } else if rssi > -80 {
                self.widgets.lbl_rssi.set_text(&format!("{} dBm (GOOD)", rssi));
                ctx_rssi.add_class("rssi-label");
            } else if rssi > -95 {
                self.widgets.lbl_rssi.set_text(&format!("{} dBm (FAIR)", rssi));
                ctx_rssi.add_class("rssi-weak");
            } else {
                self.widgets.lbl_rssi.set_text(&format!("{} dBm (WEAK)", rssi));
                ctx_rssi.add_class("rssi-weak");
            }
        } else {
            self.widgets.lbl_status_badge.set_text("OFF");
            ctx_badge.add_class("rssi-off");
            self.widgets.lbl_rssi.set_text("-- dBm");
            ctx_rssi.add_class("rssi-off");
        }

        // Volume & Route
        self.widgets.lbl_vol.set_text(&format!("🔊 Volume: {}%", vol));
        self.widgets.slider_vol.set_value(vol as f64);

        let ctx_hp = self.widgets.btn_route_hp.style_context();
        let ctx_spk = self.widgets.btn_route_spk.style_context();
        if route == "speaker" {
            ctx_hp.remove_class("route-btn-active");
            ctx_spk.add_class("route-btn-active");
        } else {
            ctx_spk.remove_class("route-btn-active");
            ctx_hp.add_class("route-btn-active");
        }

        self.state.borrow_mut().is_updating_ui = false;
    }

    fn render_presets(self: &Rc<Self>) {
        let (presets, current_freq) = {
            let st = self.state.borrow();
            (st.presets.clone(), st.current_freq)
        };

        // Clear children
        for child in self.widgets.flow_presets.children() {
            self.widgets.flow_presets.remove(&child);
        }

        for &freq in &presets {
            let btn = Button::with_label(&format!("{:.1}", freq));
            let is_active = (current_freq - freq).abs() < 0.05;
            let ctx = btn.style_context();
            if is_active {
                ctx.add_class("preset-btn-active");
            } else {
                ctx.add_class("preset-btn");
            }

            let app_ctx = Rc::clone(self);
            btn.connect_clicked(move |_| {
                app_ctx.tune_to(freq);
            });

            self.widgets.flow_presets.add(&btn);
        }
        self.widgets.flow_presets.show_all();
    }

    fn tune_to(self: &Rc<Self>, freq: f32) {
        let mut st = self.state.borrow_mut();
        st.current_freq = (freq * 10.0).round() / 10.0;
        let playing = st.is_playing;
        let cur = st.current_freq;
        drop(st);

        if playing {
            protocol::send_cmd(&format!("TUNE {:.1}", cur), 3);
        } else {
            protocol::send_cmd(&format!("START {:.1}", cur), 4);
            self.state.borrow_mut().is_playing = true;
        }

        self.sync_ui();
        self.render_presets();
    }

    fn toggle_power(self: &Rc<Self>) {
        let (playing, freq) = {
            let st = self.state.borrow();
            (st.is_playing, st.current_freq)
        };

        if playing {
            protocol::send_cmd("STOP", 3);
            self.state.borrow_mut().is_playing = false;
        } else {
            protocol::send_cmd(&format!("START {:.1}", freq), 4);
            self.state.borrow_mut().is_playing = true;
        }

        self.sync_ui();
        self.render_presets();
    }

    fn step_freq(self: &Rc<Self>, delta: f32) {
        let cur = self.state.borrow().current_freq;
        let new_f = ((cur + delta) * 10.0).round() / 10.0;
        if (87.5..=108.0).contains(&new_f) {
            self.tune_to(new_f);
        }
    }

    fn do_seek(self: &Rc<Self>, dir_up: bool) {
        let (playing, freq) = {
            let st = self.state.borrow();
            (st.is_playing, st.current_freq)
        };

        if !playing {
            protocol::send_cmd(&format!("START {:.1}", freq), 4);
            self.state.borrow_mut().is_playing = true;
        }

        self.widgets.btn_seek_down.set_sensitive(false);
        self.widgets.btn_seek_up.set_sensitive(false);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let cmd = if dir_up { "SEEK UP" } else { "SEEK DOWN" };
            let resp = protocol::send_cmd(cmd, 8);
            let _ = tx.send(resp);
        });

        let app_ctx = Rc::clone(self);
        glib::idle_add_local(move || {
            if let Ok(Some(resp)) = rx.try_recv() {
                if resp.starts_with("OK") {
                    let parts: Vec<&str> = resp.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(f) = parts[1].parse::<f32>() {
                            app_ctx.state.borrow_mut().current_freq = f;
                        }
                    }
                }
                app_ctx.widgets.btn_seek_down.set_sensitive(true);
                app_ctx.widgets.btn_seek_up.set_sensitive(true);
                app_ctx.sync_ui();
                app_ctx.render_presets();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn do_scan(self: &Rc<Self>) {
        let is_scanning = self.state.borrow().is_scanning;
        if is_scanning {
            return;
        }

        let (playing, freq) = {
            let st = self.state.borrow();
            (st.is_playing, st.current_freq)
        };

        if !playing {
            protocol::send_cmd(&format!("START {:.1}", freq), 4);
            self.state.borrow_mut().is_playing = true;
        }

        self.state.borrow_mut().is_scanning = true;
        self.widgets.btn_scan.set_label("⏳ Scanning (~30s)...");
        self.widgets.btn_scan.set_sensitive(false);
        self.sync_ui();

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let resp = protocol::send_cmd("SCAN", 60);
            let _ = tx.send(resp);
        });

        let app_ctx = Rc::clone(self);
        glib::idle_add_local(move || {
            if let Ok(resp_opt) = rx.try_recv() {
                let mut new_stations = Vec::new();
                if let Some(resp) = resp_opt {
                    if let (Some(start), Some(end)) = (resp.find('['), resp.rfind(']')) {
                        let json_part = &resp[start..=end];
                        #[derive(serde::Deserialize)]
                        struct ScanItem {
                            freq: f32,
                        }
                        if let Ok(items) = serde_json::from_str::<Vec<ScanItem>>(json_part) {
                            for item in items {
                                new_stations.push(item.freq);
                            }
                        }
                    }
                }

                let mut st = app_ctx.state.borrow_mut();
                st.is_scanning = false;
                if !new_stations.is_empty() {
                    st.presets.extend(new_stations);
                    presets::save_presets(&st.presets);
                    st.presets = presets::load_presets();
                }
                drop(st);

                app_ctx.widgets.btn_scan.set_label("🔍 Auto Scan");
                app_ctx.widgets.btn_scan.set_sensitive(true);
                app_ctx.sync_ui();
                app_ctx.render_presets();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn add_current_preset(self: &Rc<Self>) {
        let mut st = self.state.borrow_mut();
        let cur = st.current_freq;
        if !st.presets.iter().any(|&f| (f - cur).abs() < 0.05) {
            st.presets.push(cur);
            presets::save_presets(&st.presets);
            st.presets = presets::load_presets();
        }
        drop(st);
        self.render_presets();
    }
}

fn build_ui(app: &Application) {
    let screen = gtk::gdk::Screen::default().expect("Failed to get default screen");
    let provider = CssProvider::new();
    provider.load_from_data(CSS.as_bytes()).expect("Failed to load CSS");
    StyleContext::add_provider_for_screen(
        &screen,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = ApplicationWindow::builder()
        .application(app)
        .title("FM Radio")
        .default_width(540)
        .default_height(800)
        .window_position(WindowPosition::Center)
        .build();

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .build();

    let root_box = GtkBox::new(Orientation::Vertical, 4);

    // ==========================================
    // 1. Header Bar
    // ==========================================
    let header_box = GtkBox::new(Orientation::Horizontal, 12);
    header_box.set_margin_top(14);
    header_box.set_margin_bottom(6);
    header_box.set_margin_start(16);
    header_box.set_margin_end(16);

    let lbl_title = Label::new(Some("FM Radio"));
    lbl_title.style_context().add_class("header-title");

    let lbl_status_badge = Label::new(Some("OFF"));
    lbl_status_badge.style_context().add_class("rssi-off");

    let close_btn = Button::with_label("✕");
    close_btn.style_context().add_class("close-btn");
    let win_clone = window.clone();
    close_btn.connect_clicked(move |_| {
        win_clone.close();
    });

    header_box.pack_start(&lbl_title, false, false, 0);
    header_box.pack_start(&lbl_status_badge, false, false, 8);
    header_box.pack_end(&close_btn, false, false, 0);
    root_box.pack_start(&header_box, false, false, 0);

    // ==========================================
    // 2. Frequency Display Card
    // ==========================================
    let card_freq = GtkBox::new(Orientation::Vertical, 8);
    card_freq.style_context().add_class("card");

    let row_freq = GtkBox::new(Orientation::Horizontal, 8);
    row_freq.set_halign(gtk::Align::Center);

    let lbl_freq = Label::new(Some("98.3"));
    lbl_freq.style_context().add_class("freq-display");

    let lbl_mhz = Label::new(Some("MHz"));
    lbl_mhz.style_context().add_class("mhz-label");
    lbl_mhz.set_valign(gtk::Align::End);
    lbl_mhz.set_margin_bottom(12);

    row_freq.pack_start(&lbl_freq, false, false, 0);
    row_freq.pack_start(&lbl_mhz, false, false, 0);
    card_freq.pack_start(&row_freq, false, false, 0);

    let lbl_rssi = Label::new(Some("-- dBm"));
    lbl_rssi.style_context().add_class("rssi-off");
    lbl_rssi.set_halign(gtk::Align::Center);
    card_freq.pack_start(&lbl_rssi, false, false, 0);

    root_box.pack_start(&card_freq, false, false, 0);

    // ==========================================
    // 3. Tuning & Power Card
    // ==========================================
    let card_tune = GtkBox::new(Orientation::Vertical, 16);
    card_tune.style_context().add_class("card");

    let slider_freq = Scale::with_range(Orientation::Horizontal, 87.5, 108.0, 0.1);
    slider_freq.set_draw_value(false);
    slider_freq.set_value(98.3);
    card_tune.pack_start(&slider_freq, false, false, 4);

    let row_tune_btns = GtkBox::new(Orientation::Horizontal, 8);
    row_tune_btns.set_homogeneous(true);

    let btn_seek_down = Button::with_label("⏮ Seek");
    btn_seek_down.style_context().add_class("tune-btn");

    let btn_step_down = Button::with_label("-0.1");
    btn_step_down.style_context().add_class("tune-btn");

    let btn_step_up = Button::with_label("+0.1");
    btn_step_up.style_context().add_class("tune-btn");

    let btn_seek_up = Button::with_label("Seek ⏭");
    btn_seek_up.style_context().add_class("tune-btn");

    row_tune_btns.pack_start(&btn_seek_down, true, true, 0);
    row_tune_btns.pack_start(&btn_step_down, true, true, 0);
    row_tune_btns.pack_start(&btn_step_up, true, true, 0);
    row_tune_btns.pack_start(&btn_seek_up, true, true, 0);
    card_tune.pack_start(&row_tune_btns, false, false, 0);

    let btn_power = Button::with_label("⚡  POWER ON");
    btn_power.style_context().add_class("power-btn-off");
    card_tune.pack_start(&btn_power, false, false, 4);

    root_box.pack_start(&card_tune, false, false, 0);

    // ==========================================
    // 4. Volume & Audio Route Card
    // ==========================================
    let card_audio = GtkBox::new(Orientation::Vertical, 12);
    card_audio.style_context().add_class("card");

    let row_vol = GtkBox::new(Orientation::Horizontal, 12);
    let lbl_vol = Label::new(Some("🔊 Volume: 75%"));
    lbl_vol.style_context().add_class("header-title");
    lbl_vol.set_halign(gtk::Align::Start);
    row_vol.pack_start(&lbl_vol, true, true, 0);

    let btn_route_hp = Button::with_label("🎧 Headphone");
    btn_route_hp.style_context().add_class("route-btn-active");

    let btn_route_spk = Button::with_label("🔊 Speaker");
    btn_route_spk.style_context().add_class("route-btn");

    row_vol.pack_end(&btn_route_spk, false, false, 0);
    row_vol.pack_end(&btn_route_hp, false, false, 0);
    card_audio.pack_start(&row_vol, false, false, 0);

    let slider_vol = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    slider_vol.set_draw_value(false);
    slider_vol.set_value(75.0);
    card_audio.pack_start(&slider_vol, false, false, 0);

    root_box.pack_start(&card_audio, false, false, 0);

    // ==========================================
    // 5. Presets & Auto Scan Card
    // ==========================================
    let card_presets = GtkBox::new(Orientation::Vertical, 12);
    card_presets.style_context().add_class("card");

    let row_preset_header = GtkBox::new(Orientation::Horizontal, 8);
    let lbl_preset_title = Label::new(Some("⭐ Saved Stations"));
    lbl_preset_title.style_context().add_class("header-title");

    let btn_add_preset = Button::with_label("+ Save Preset");
    btn_add_preset.style_context().add_class("route-btn");

    row_preset_header.pack_start(&lbl_preset_title, false, false, 0);
    row_preset_header.pack_end(&btn_add_preset, false, false, 0);
    card_presets.pack_start(&row_preset_header, false, false, 0);

    let flow_presets = FlowBox::new();
    flow_presets.set_selection_mode(gtk::SelectionMode::None);
    flow_presets.set_max_children_per_line(4);
    flow_presets.set_min_children_per_line(3);
    flow_presets.set_column_spacing(8);
    flow_presets.set_row_spacing(8);
    card_presets.pack_start(&flow_presets, false, false, 4);

    let btn_scan = Button::with_label("🔍 Auto Scan");
    btn_scan.style_context().add_class("route-btn");
    btn_scan.set_margin_top(8);
    card_presets.pack_start(&btn_scan, false, false, 0);

    root_box.pack_start(&card_presets, false, false, 0);

    scroll.add(&root_box);
    window.add(&scroll);

    // Initialize State & AppContext
    protocol::ensure_daemon();
    let initial_presets = presets::load_presets();

    let state = AppState {
        current_freq: 98.3,
        is_playing: false,
        current_rssi: -100,
        current_route: "headphone".to_string(),
        current_vol: 75,
        presets: initial_presets,
        is_updating_ui: false,
        is_scanning: false,
    };

    let widgets = AppWidgets {
        lbl_status_badge,
        lbl_freq,
        lbl_rssi,
        slider_freq: slider_freq.clone(),
        btn_seek_down: btn_seek_down.clone(),
        btn_step_down,
        btn_step_up,
        btn_seek_up: btn_seek_up.clone(),
        btn_power: btn_power.clone(),
        slider_vol: slider_vol.clone(),
        lbl_vol,
        btn_route_hp: btn_route_hp.clone(),
        btn_route_spk: btn_route_spk.clone(),
        flow_presets,
        btn_scan: btn_scan.clone(),
    };

    let app_ctx = Rc::new(AppContext {
        state: RefCell::new(state),
        widgets,
    });

    // Initial status query
    if let Some(resp) = protocol::send_cmd("STATUS", 2) {
        if let Some(st) = protocol::parse_status(&resp) {
            let mut s = app_ctx.state.borrow_mut();
            s.is_playing = st.is_playing;
            s.current_freq = st.freq;
            s.current_rssi = st.rssi;
            s.current_route = st.route;
            s.current_vol = st.vol;
        }
    }

    // Connect Callbacks
    let ctx_pwr = Rc::clone(&app_ctx);
    btn_power.connect_clicked(move |_| {
        ctx_pwr.toggle_power();
    });

    let ctx_step_dn = Rc::clone(&app_ctx);
    app_ctx.widgets.btn_step_down.connect_clicked(move |_| {
        ctx_step_dn.step_freq(-0.1);
    });

    let ctx_step_up = Rc::clone(&app_ctx);
    app_ctx.widgets.btn_step_up.connect_clicked(move |_| {
        ctx_step_up.step_freq(0.1);
    });

    let ctx_seek_dn = Rc::clone(&app_ctx);
    btn_seek_down.connect_clicked(move |_| {
        ctx_seek_dn.do_seek(false);
    });

    let ctx_seek_up = Rc::clone(&app_ctx);
    btn_seek_up.connect_clicked(move |_| {
        ctx_seek_up.do_seek(true);
    });

    let ctx_scale = Rc::clone(&app_ctx);
    slider_freq.connect_value_changed(move |scale| {
        let is_updating = ctx_scale.state.borrow().is_updating_ui;
        if is_updating {
            return;
        }
        let val = (scale.value() * 10.0).round() as f32 / 10.0;
        let cur = ctx_scale.state.borrow().current_freq;
        if (val - cur).abs() >= 0.09 {
            ctx_scale.tune_to(val);
        }
    });

    let ctx_vol = Rc::clone(&app_ctx);
    slider_vol.connect_value_changed(move |scale| {
        let is_updating = ctx_vol.state.borrow().is_updating_ui;
        if is_updating {
            return;
        }
        let v = scale.value().round() as i32;
        ctx_vol.state.borrow_mut().current_vol = v;
        protocol::send_cmd(&format!("VOL {}", v), 2);
        ctx_vol.widgets.lbl_vol.set_text(&format!("🔊 Volume: {}%", v));
    });

    let ctx_hp = Rc::clone(&app_ctx);
    btn_route_hp.connect_clicked(move |_| {
        ctx_hp.state.borrow_mut().current_route = "headphone".to_string();
        protocol::send_cmd("ROUTE headphone", 2);
        ctx_hp.sync_ui();
    });

    let ctx_spk = Rc::clone(&app_ctx);
    btn_route_spk.connect_clicked(move |_| {
        ctx_spk.state.borrow_mut().current_route = "speaker".to_string();
        protocol::send_cmd("ROUTE speaker", 2);
        ctx_spk.sync_ui();
    });

    let ctx_add = Rc::clone(&app_ctx);
    btn_add_preset.connect_clicked(move |_| {
        ctx_add.add_current_preset();
    });

    let ctx_scan = Rc::clone(&app_ctx);
    btn_scan.connect_clicked(move |_| {
        ctx_scan.do_scan();
    });

    // 1.2-second periodic background status poll
    let ctx_poll = Rc::clone(&app_ctx);
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        if !ctx_poll.state.borrow().is_scanning {
            if let Some(resp) = protocol::send_cmd("STATUS", 1) {
                if let Some(st) = protocol::parse_status(&resp) {
                    let mut s = ctx_poll.state.borrow_mut();
                    let freq_changed = (s.current_freq - st.freq).abs() >= 0.09;
                    s.is_playing = st.is_playing;
                    s.current_rssi = st.rssi;
                    s.current_route = st.route;
                    s.current_vol = st.vol;
                    if freq_changed {
                        s.current_freq = st.freq;
                    }
                    drop(s);
                    if freq_changed {
                        ctx_poll.render_presets();
                    }
                }
            }
            ctx_poll.sync_ui();
        }
        glib::ControlFlow::Continue
    });

    app_ctx.sync_ui();
    app_ctx.render_presets();

    window.show_all();
}

fn main() {
    let app = Application::builder()
        .application_id("org.blossom.fmradio")
        .build();

    app.connect_activate(build_ui);
    app.run();
}
