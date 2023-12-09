use egui::{Context, Window};
use futures::executor::block_on;
use ron::ser::PrettyConfig;

use automancy::game::GameMsg;

use crate::event::EventLoopStorage;
use crate::gui::default_frame;
use crate::renderer::Renderer;
use crate::setup::GameSetup;

/// Draws the debug menu (F3).
pub fn debugger(
    setup: &GameSetup,
    loop_store: &mut EventLoopStorage,
    renderer: &Renderer,
    context: &Context,
) {
    let resource_man = setup.resource_man.clone();

    let fps = 1.0 / loop_store.elapsed.as_secs_f64();

    let reg_tiles = resource_man.registry.tiles.len();
    let reg_items = resource_man.registry.items.len();
    let tags = resource_man.registry.tags.len();
    let functions = resource_man.functions.len();
    let scripts = resource_man.registry.scripts.len();
    let audio = resource_man.audio.len();
    let meshes = resource_man.all_models.len();

    let (info, map_name) = block_on(setup.game.call(GameMsg::GetMapInfo, None))
        .unwrap()
        .unwrap();

    let tile_count = info.tile_count;

    Window::new(
        setup.resource_man.translates.gui[&resource_man.registry.gui_ids.debug_menu].as_str(),
    )
    .resizable(false)
    .default_width(600.0)
    .frame(default_frame())
    .show(context, |ui| {
        ui.label(format!("FPS: {fps:.1}"));
        ui.label(format!("WGPU: {}", ron::ser::to_string_pretty(&renderer.gpu.adapter_info, PrettyConfig::default()).unwrap_or("could not format info".to_string())));
        ui.separator();
        ui.label(format!(
            "ResourceMan: Tiles={reg_tiles} Items={reg_items} Tags={tags} Functions={functions} Scripts={scripts} Audio={audio} Meshes={meshes}"
        ));
        ui.label(format!("Map \"{map_name}\": Tiles={tile_count}"))
    });
}
