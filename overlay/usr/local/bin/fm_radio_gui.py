#!/usr/bin/env python3
"""
FM Radio - Android-style Touchscreen FM Radio for Sxmo / Linux
Designed for Xiaomi Redmi 9A (720x1600 touch display)
"""

import os
import sys
import time
import socket
import subprocess
import threading
import json
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gtk, Gdk, GLib, Pango

SOCK_PATH = "/run/mtk_fm.sock"
PRESETS_FILE = os.path.expanduser("~/.config/fm_presets.json")
DEFAULT_PRESETS = [88.6, 96.6, 97.2, 97.6, 98.3, 100.4, 104.4]

CSS = b"""
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
}

.rssi-off {
    color: #8da0b8;
    background-color: rgba(141, 160, 184, 0.12);
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
    border: 1px solid #00e5ff;
}

.route-btn {
    background-color: #242c38;
    color: #b0bec5;
    font-size: 13px;
    font-weight: 700;
    border-radius: 12px;
    padding: 6px 12px;
    border: 1px solid #374354;
}

.route-btn-active {
    background-color: #00e5ff;
    color: #000000;
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
"""

def send_cmd(cmd, timeout=3.0):
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(timeout)
        s.connect(SOCK_PATH)
        s.sendall((cmd + "\n").encode())
        resp = s.recv(4096).decode().strip()
        s.close()
        return resp
    except Exception:
        return None

def ensure_daemon():
    resp = send_cmd("STATUS")
    if resp is None:
        subprocess.Popen(["/usr/local/bin/mtk_fm", "--daemon"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.4)

class FMRadioApp(Gtk.Window):
    def __init__(self):
        super().__init__(title="FM Radio")
        self.set_default_size(540, 800)
        self.set_position(Gtk.WindowPosition.CENTER)

        ensure_daemon()
        self.load_presets()

        self.current_freq = 98.3
        self.is_playing = False
        self.current_rssi = -100
        self.current_route = "headphone"
        self.current_vol = 75
        self.updating_ui = False
        self.is_scanning = False

        self.build_ui()
        self.sync_state()

        # 1.2-second background refresh timer
        GLib.timeout_add(1200, self.poll_status)

    def load_presets(self):
        self.presets = list(DEFAULT_PRESETS)
        try:
            if os.path.exists(PRESETS_FILE):
                with open(PRESETS_FILE, "r") as f:
                    data = json.load(f)
                    if isinstance(data, list) and len(data) > 0:
                        self.presets = sorted(list(set(data)))
        except Exception:
            pass

    def save_presets(self):
        try:
            os.makedirs(os.path.dirname(PRESETS_FILE), exist_ok=True)
            with open(PRESETS_FILE, "w") as f:
                json.dump(self.presets, f)
        except Exception:
            pass

    def build_ui(self):
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.add(main_box)

        # Header bar
        header_card = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        header_card.get_style_context().add_class("card")

        lbl_icon = Gtk.Label(label="📻")
        lbl_icon.get_style_context().add_class("header-title")
        lbl_title = Gtk.Label(label="FM Radio")
        lbl_title.get_style_context().add_class("header-title")
        lbl_title.set_hexpand(True)
        lbl_title.set_xalign(0.0)

        self.btn_route_hp = Gtk.Button(label="🎧 Headphones")
        self.btn_route_hp.get_style_context().add_class("route-btn")
        self.btn_route_hp.connect("clicked", lambda b: self.set_route("headphone"))

        self.btn_route_spk = Gtk.Button(label="🔊 Speaker")
        self.btn_route_spk.get_style_context().add_class("route-btn")
        self.btn_route_spk.connect("clicked", lambda b: self.set_route("speaker"))

        btn_close = Gtk.Button(label="✕")
        btn_close.get_style_context().add_class("close-btn")
        btn_close.connect("clicked", lambda b: self.destroy())

        header_card.pack_start(lbl_icon, False, False, 0)
        header_card.pack_start(lbl_title, True, True, 0)
        header_card.pack_start(self.btn_route_hp, False, False, 0)
        header_card.pack_start(self.btn_route_spk, False, False, 0)
        header_card.pack_start(btn_close, False, False, 0)
        main_box.pack_start(header_card, False, False, 0)

        # Main Tuner Display Card
        display_card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        display_card.get_style_context().add_class("card")

        # Top status row: Antenna / RDS indicator + RSSI Badge
        status_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.lbl_status_badge = Gtk.Label(label="RADIO OFF")
        self.lbl_status_badge.get_style_context().add_class("rssi-label")
        self.lbl_status_badge.get_style_context().add_class("rssi-off")

        self.lbl_rssi = Gtk.Label(label="-- dBm")
        self.lbl_rssi.get_style_context().add_class("rssi-label")
        self.lbl_rssi.get_style_context().add_class("rssi-off")

        status_row.pack_start(self.lbl_status_badge, False, False, 0)
        status_row.pack_end(self.lbl_rssi, False, False, 0)
        display_card.pack_start(status_row, False, False, 0)

        # Large Digital Frequency readout
        freq_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        freq_box.set_halign(Gtk.Align.CENTER)

        self.lbl_freq = Gtk.Label(label="98.3")
        self.lbl_freq.get_style_context().add_class("freq-display")
        lbl_mhz = Gtk.Label(label="MHz")
        lbl_mhz.get_style_context().add_class("mhz-label")
        lbl_mhz.set_valign(Gtk.Align.END)
        lbl_mhz.set_margin_bottom(14)

        freq_box.pack_start(self.lbl_freq, False, False, 0)
        freq_box.pack_start(lbl_mhz, False, False, 0)
        display_card.pack_start(freq_box, False, False, 4)

        # Tuning Frequency Slider Scale
        self.scale_freq = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 87.5, 108.0, 0.1)
        self.scale_freq.set_value(98.3)
        self.scale_freq.set_draw_value(False)
        self.scale_freq.connect("value-changed", self.on_slider_changed)
        display_card.pack_start(self.scale_freq, False, False, 6)

        # Fine tuning and Seek Controls
        tune_controls = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        tune_controls.set_halign(Gtk.Align.CENTER)

        self.btn_seek_down = Gtk.Button(label="◀◀ Seek")
        self.btn_seek_down.get_style_context().add_class("tune-btn")
        self.btn_seek_down.connect("clicked", lambda b: self.do_seek(-1))

        btn_step_down = Gtk.Button(label="- 0.1")
        btn_step_down.get_style_context().add_class("tune-btn")
        btn_step_down.connect("clicked", lambda b: self.step_freq(-0.1))

        btn_step_up = Gtk.Button(label="+ 0.1")
        btn_step_up.get_style_context().add_class("tune-btn")
        btn_step_up.connect("clicked", lambda b: self.step_freq(0.1))

        self.btn_seek_up = Gtk.Button(label="Seek ▶▶")
        self.btn_seek_up.get_style_context().add_class("tune-btn")
        self.btn_seek_up.connect("clicked", lambda b: self.do_seek(1))

        tune_controls.pack_start(self.btn_seek_down, False, False, 0)
        tune_controls.pack_start(btn_step_down, False, False, 0)
        tune_controls.pack_start(btn_step_up, False, False, 0)
        tune_controls.pack_start(self.btn_seek_up, False, False, 0)
        display_card.pack_start(tune_controls, False, False, 6)

        main_box.pack_start(display_card, False, False, 0)

        # Power Action Button Card
        power_card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        power_card.get_style_context().add_class("card")

        self.btn_power = Gtk.Button(label="⏻ TURN ON")
        self.btn_power.get_style_context().add_class("power-btn-off")
        self.btn_power.connect("clicked", self.toggle_power)
        power_card.pack_start(self.btn_power, False, False, 0)
        main_box.pack_start(power_card, False, False, 0)

        # Volume & Presets Card
        control_card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        control_card.get_style_context().add_class("card")

        # Volume slider
        vol_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        lbl_vol_icon = Gtk.Label(label="🔊")
        self.scale_vol = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 5)
        self.scale_vol.set_value(75)
        self.scale_vol.set_hexpand(True)
        self.scale_vol.set_draw_value(False)
        self.scale_vol.connect("value-changed", self.on_vol_changed)

        vol_box.pack_start(lbl_vol_icon, False, False, 0)
        vol_box.pack_start(self.scale_vol, True, True, 0)
        control_card.pack_start(vol_box, False, False, 0)

        # Preset Channels row
        presets_header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        lbl_presets = Gtk.Label(label="Station Presets")
        lbl_presets.get_style_context().add_class("header-title")
        lbl_presets.set_hexpand(True)
        lbl_presets.set_xalign(0.0)

        self.btn_scan = Gtk.Button(label="🔍 Auto Scan")
        self.btn_scan.get_style_context().add_class("route-btn")
        self.btn_scan.connect("clicked", self.start_auto_scan)

        btn_add_preset = Gtk.Button(label="+ Favorite")
        btn_add_preset.get_style_context().add_class("route-btn")
        btn_add_preset.connect("clicked", self.add_current_preset)

        presets_header.pack_start(lbl_presets, True, True, 0)
        presets_header.pack_start(btn_add_preset, False, False, 0)
        presets_header.pack_start(self.btn_scan, False, False, 0)
        control_card.pack_start(presets_header, False, False, 4)

        # Horizontal scroll for presets
        self.presets_scroll = Gtk.ScrolledWindow()
        self.presets_scroll.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.NEVER)
        self.presets_scroll.set_min_content_height(54)

        self.presets_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.presets_scroll.add(self.presets_box)
        control_card.pack_start(self.presets_scroll, False, False, 0)

        main_box.pack_start(control_card, True, True, 0)
        self.render_presets()

    def render_presets(self):
        for child in self.presets_box.get_children():
            self.presets_box.remove(child)

        for p in self.presets:
            btn = Gtk.Button(label=f"{p:.1f}")
            btn.get_style_context().add_class("preset-btn")
            if abs(p - self.current_freq) < 0.05 and self.is_playing:
                btn.get_style_context().add_class("preset-btn-active")
            btn.connect("clicked", lambda b, freq=p: self.tune_to(freq))
            self.presets_box.pack_start(btn, False, False, 0)
        self.presets_box.show_all()

    def sync_state(self):
        if self.is_scanning:
            return
        resp = send_cmd("STATUS")
        if not resp:
            return
        parts = resp.split()
        if len(parts) >= 1:
            state = parts[0]
            if state == "PLAYING" and len(parts) >= 5:
                self.is_playing = True
                self.current_freq = float(parts[1])
                self.current_rssi = int(parts[2])
                self.current_route = parts[3]
                self.current_vol = int(parts[4])
            else:
                self.is_playing = False
                if len(parts) >= 2:
                    try:
                        self.current_freq = float(parts[1])
                    except Exception:
                        pass
        self.update_ui_state()

    def update_ui_state(self):
        self.updating_ui = True
        self.lbl_freq.set_text(f"{self.current_freq:.1f}")
        self.scale_freq.set_value(self.current_freq)
        self.scale_vol.set_value(self.current_vol)

        ctx_power = self.btn_power.get_style_context()
        ctx_badge = self.lbl_status_badge.get_style_context()
        ctx_rssi = self.lbl_rssi.get_style_context()

        if self.is_playing:
            self.btn_power.set_label("⏻ TURN OFF (PLAYING)")
            ctx_power.remove_class("power-btn-off")
            ctx_power.add_class("power-btn-on")

            self.lbl_status_badge.set_text("LIVE ON AIR")
            ctx_badge.remove_class("rssi-off")
            ctx_badge.remove_class("rssi-weak")
            ctx_badge.add_class("rssi-label")

            self.lbl_rssi.set_text(f"📶 {self.current_rssi} dBm")
            ctx_rssi.remove_class("rssi-off")
            if self.current_rssi < -95:
                ctx_rssi.add_class("rssi-weak")
            else:
                ctx_rssi.remove_class("rssi-weak")
                ctx_rssi.add_class("rssi-label")
        else:
            self.btn_power.set_label("⏻ TURN ON")
            ctx_power.remove_class("power-btn-on")
            ctx_power.add_class("power-btn-off")

            self.lbl_status_badge.set_text("RADIO OFF")
            ctx_badge.remove_class("rssi-label")
            ctx_badge.remove_class("rssi-weak")
            ctx_badge.add_class("rssi-off")

            self.lbl_rssi.set_text("-- dBm")
            ctx_rssi.remove_class("rssi-label")
            ctx_rssi.remove_class("rssi-weak")
            ctx_rssi.add_class("rssi-off")

        # Route buttons
        ctx_hp = self.btn_route_hp.get_style_context()
        ctx_spk = self.btn_route_spk.get_style_context()
        if self.current_route == "speaker":
            ctx_hp.remove_class("route-btn-active")
            ctx_spk.add_class("route-btn-active")
        else:
            ctx_spk.remove_class("route-btn-active")
            ctx_hp.add_class("route-btn-active")

        self.render_presets()
        self.updating_ui = False

    def toggle_power(self, btn):
        if self.is_playing:
            send_cmd("STOP")
            self.is_playing = False
        else:
            send_cmd(f"START {self.current_freq:.1f}")
            self.is_playing = True
        self.sync_state()

    def tune_to(self, freq):
        self.current_freq = round(freq, 1)
        if self.is_playing:
            send_cmd(f"TUNE {self.current_freq:.1f}")
        else:
            send_cmd(f"START {self.current_freq:.1f}")
            self.is_playing = True
        self.sync_state()

    def step_freq(self, delta):
        new_f = round(self.current_freq + delta, 1)
        if 87.5 <= new_f <= 108.0:
            self.tune_to(new_f)

    def do_seek(self, direction):
        if not self.is_playing:
            self.toggle_power(None)
            time.sleep(0.2)

        self.btn_seek_down.set_sensitive(False)
        self.btn_seek_up.set_sensitive(False)

        def _seek_worker():
            cmd = "SEEK UP" if direction > 0 else "SEEK DOWN"
            resp = send_cmd(cmd, timeout=8.0)
            if resp and resp.startswith("OK"):
                parts = resp.split()
                if len(parts) >= 2:
                    try:
                        self.current_freq = float(parts[1])
                    except Exception:
                        pass
            GLib.idle_add(self._seek_done)

        threading.Thread(target=_seek_worker, daemon=True).start()

    def _seek_done(self):
        self.btn_seek_down.set_sensitive(True)
        self.btn_seek_up.set_sensitive(True)
        self.sync_state()

    def on_slider_changed(self, scale):
        if self.updating_ui:
            return
        val = round(scale.get_value(), 1)
        if abs(val - self.current_freq) >= 0.09:
            self.tune_to(val)

    def on_vol_changed(self, scale):
        if self.updating_ui:
            return
        v = int(scale.get_value())
        self.current_vol = v
        send_cmd(f"VOL {v}")

    def set_route(self, route):
        self.current_route = route
        send_cmd(f"ROUTE {route}")
        self.sync_state()

    def add_current_preset(self):
        f = round(self.current_freq, 1)
        if f not in self.presets:
            self.presets.append(f)
            self.presets.sort()
            self.save_presets()
            self.render_presets()

    def start_auto_scan(self, btn):
        if self.is_scanning:
            return
        if not self.is_playing:
            self.toggle_power(None)
            time.sleep(0.3)

        self.is_scanning = True
        self.btn_scan.set_label("⏳ Scanning (~30s)...")
        self.btn_scan.set_sensitive(False)
        self.lbl_status_badge.set_text("SCANNING...")

        threading.Thread(target=self._run_scan_worker, daemon=True).start()

    def _run_scan_worker(self):
        resp = send_cmd("SCAN", timeout=60.0)
        new_stations = []
        if resp and "OK SCAN" in resp:
            try:
                json_part = resp[resp.find("["):resp.rfind("]")+1]
                data = json.loads(json_part)
                new_stations = [item["freq"] for item in data if "freq" in item]
            except Exception as e:
                print("JSON scan error:", e)

        if new_stations:
            self.presets = sorted(list(set(self.presets + new_stations)))
            self.save_presets()

        GLib.idle_add(self._scan_done)

    def _scan_done(self):
        self.is_scanning = False
        self.btn_scan.set_label("🔍 Auto Scan")
        self.btn_scan.set_sensitive(True)
        self.sync_state()
        self.render_presets()

    def poll_status(self):
        if not self.is_scanning:
            self.sync_state()
        return True

def main():
    screen = Gdk.Screen.get_default()
    css_provider = Gtk.CssProvider()
    css_provider.load_from_data(CSS)
    Gtk.StyleContext.add_provider_for_screen(
        screen, css_provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
    )

    app = FMRadioApp()
    app.connect("destroy", Gtk.main_quit)
    app.show_all()
    Gtk.main()

if __name__ == "__main__":
    main()
