//! Standalone gain-style UI (no audio). Pattern for nih-plug param rows.

use repose_audui::baseview_host::{ReposeBaseviewConfig, open_blocking};
use repose_audui::param_widgets::{panel, section_title};
use repose_core::prelude::*;
use repose_material::material3::{Slider, SliderConfig};
use repose_ui::{Text, TextStyle};

fn main() {
    let _ = env_logger::try_init();
    open_blocking(
        ReposeBaseviewConfig {
            title: "Gain (Repose)".into(),
            logical_size: (420.0, 200.0),
            ..Default::default()
        },
        |_s, _c| {
            let gain = remember_mutable(|| 0.75f32);
            panel((
                section_title("Gain"),
                Text(format!("{:.0}%", *gain.get() * 100.0)).size(14.0.sp()),
                Slider(
                    *gain.get(),
                    (0.0, 1.0),
                    Some(0.01),
                    {
                        let gain = gain.clone();
                        move |v| gain.set(v)
                    },
                    SliderConfig::default(),
                ),
            ))
        },
    );
}
