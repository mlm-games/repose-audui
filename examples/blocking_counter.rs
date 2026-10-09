use repose_audui::baseview_host::{ReposeBaseviewConfig, open_blocking};
use repose_core::prelude::*;
use repose_ui::{Column, Text, TextStyle, ViewExt};

fn main() {
    let _ = env_logger::try_init();
    open_blocking(
        ReposeBaseviewConfig {
            title: "repose-audui counter".into(),
            logical_size: (360.0, 240.0),
            ..Default::default()
        },
        |_s, _c| {
            let count = remember_mutable(|| 0i32);
            Column(
                Modifier::new()
                    .padding(24.0.dp())
                    .gap(12.0.dp())
                    .fill_max_size(),
            )
            .child((
                Text("Repose in baseview").size(20.0.sp()),
                Text(format!("Count: {}", *count.get())).size(16.0.sp()),
                repose_ui::Box(Modifier::new().on_click({
                    let count = count.clone();
                    move || count.update(|c| *c += 1)
                }))
                .child(Text("Increment")),
            ))
        },
    );
}
